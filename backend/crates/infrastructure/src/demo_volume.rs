//! The demo data at volume (`agri-connect demo-data`): the file's demo data
//! (`demo/data.json`) grown to the size of a real deployment, for trying
//! the pages and timing the reads on it.
//!
//! `generate` is pure: it takes the file's data, the words it is made of
//! (`demo/vocabulary.json`), the wards it may place things in and a
//! `DemoVolume`, and returns more `DemoData` of the same shape, which the
//! loader writes exactly as it writes the file's. So every generated row
//! holds to the rules the file's do (unit tests run both through the same
//! checks): a service or supply belongs to its shop, a review comes from the
//! phone of a request that shop answered, a price is one per crop, area and
//! day, a sponsored shop lies in its contract's area.
//!
//! The same seed gives the same rows: the generator draws from its own
//! SplitMix64 stream, and never iterates a hash map.
//!
//! Proportions, for `providers` = n: about n accounts (providers and
//! partners), 1.2n services, 1.1n supplies, 2n contact requests and n
//! reviews from them, n job posts, n alerts, n/25 partners, crops on several
//! area presets each (a few thousand prices), and taps and searches by
//! report week, more in the busy wards so some areas come out steady.

use std::collections::{BTreeMap, HashMap, HashSet};

use application::{
    admin_units::preset_label,
    constants::{DEMAND_WEEKS, PRICE_PER_UNIT_MAX},
    slug::slugify,
};
use domain::{models::catalogue::DemoVolume, Error, Result};
use serde::Deserialize;

use crate::{
    demo_activity::{
        DemoAccount, DemoAlert, DemoArea, DemoCalls, DemoContract, DemoInquiry, DemoJob,
        DemoPartner, DemoPrices, DemoReview, DemoSearches,
    },
    demo_data_repository::{
        DemoBusiness, DemoCoded, DemoCommodity, DemoCrop, DemoData, DemoLocation, DemoProduct,
        DemoService, DemoType,
    },
};

const VOCABULARY: &str = include_str!("demo/vocabulary.json");

/// Partners per provider: one for every this many.
const PROVIDERS_PER_PARTNER: usize = 25;
/// The part of a demo account's email before the `@`, at most (an address
/// allows 64).
const EMAIL_LOCAL_MAX: usize = 50;
/// Contact requests per provider; about half are answered, most of those
/// reviewed, which gives about one review per provider.
const INQUIRIES_PER_PROVIDER: usize = 2;
/// Farmers (names and phones) per provider: some ask several shops.
const FARMERS_PER_PROVIDER: usize = 2;
/// How far from its ward's point a shop is placed, in degrees (about 600 m).
const SHOP_JITTER_DEG: f64 = 0.006;
/// Area presets a crop has prices in.
const PRICE_AREAS: (i64, i64) = (5, 8);
/// Days back a price may be from (most older than a commodity's freshness).
const PRICE_DAYS_BACK: i64 = 45;
/// Days back a contact request, a job post, an alert may be from.
const INQUIRY_DAYS_BACK: f64 = 180.0;
const JOB_DAYS_BACK: i64 = 330;
const ALERT_DAYS_BACK: i64 = 720;

#[derive(Deserialize)]
pub(crate) struct Vocabulary {
    email_domain: String,
    phone_prefixes: Vec<String>,
    pub(crate) provinces: Vec<ProvinceShare>,
    people: People,
    shop_names: ShopNames,
    shop_descriptions: Vec<String>,
    sectors: Vec<DemoCoded>,
    alert_kinds: Vec<DemoCoded>,
    commodities: Vec<DemoCommodity>,
    crops: Vec<CropFamily>,
    service_types: Vec<DemoType>,
    service_offers: BTreeMap<String, Vec<Offer>>,
    supply_types: Vec<DemoType>,
    supply_items: BTreeMap<String, Vec<Item>>,
    farm_sizes: Vec<String>,
    when: Vec<String>,
    inquiry_notes: Vec<String>,
    job_descriptions: Vec<String>,
    reviews: ReviewWords,
    replies: Vec<String>,
    search_queries_unmet: Vec<String>,
    alerts: BTreeMap<String, Vec<AlertWords>>,
    partners: PartnerWords,
}

/// A province the generated shops lie in: its share of them, and how many
/// of its busiest wards become area presets.
#[derive(Deserialize)]
pub(crate) struct ProvinceShare {
    pub(crate) code: String,
    weight: f64,
    presets: usize,
}

#[derive(Deserialize)]
struct People {
    family: Vec<String>,
    middle_male: Vec<String>,
    middle_female: Vec<String>,
    given_male: Vec<String>,
    given_female: Vec<String>,
    rank: Vec<String>,
}

#[derive(Deserialize)]
struct ShopNames {
    service: Vec<String>,
    dealer: Vec<String>,
    both: Vec<String>,
}

/// The crops of one commodity and the range their prices move in.
#[derive(Deserialize)]
struct CropFamily {
    commodity: String,
    unit: String,
    price: (i64, i64),
    names: Vec<String>,
}

#[derive(Deserialize)]
struct Offer {
    name: String,
    description: String,
    price_note: String,
}

#[derive(Deserialize)]
struct Item {
    name: String,
    unit: String,
    price: (i64, i64),
    #[serde(default)]
    commodity: Option<String>,
}

#[derive(Deserialize)]
struct ReviewWords {
    good: Vec<String>,
    mixed: Vec<String>,
    bad: Vec<String>,
}

#[derive(Deserialize)]
struct AlertWords {
    title: String,
    body: String,
}

#[derive(Deserialize)]
struct PartnerWords {
    brand: Vec<String>,
    dealer: Vec<String>,
    cooperative: Vec<String>,
    exporter: Vec<String>,
    names: Vec<String>,
    notes: Vec<String>,
}

pub(crate) fn vocabulary() -> Result<Vocabulary> {
    serde_json::from_str(VOCABULARY)
        .map_err(|e| Error::internal(format!("demo vocabulary file: {e}")))
}

/// A ward generated rows may lie in, with its province's name.
#[derive(Clone, Debug)]
pub(crate) struct WardPlace {
    pub(crate) province_code: String,
    pub(crate) province: String,
    pub(crate) name: String,
    pub(crate) full_name: String,
    pub(crate) lat: f64,
    pub(crate) lng: f64,
}

/// SplitMix64: small, fast, and the same numbers on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in [0, n); 0 when n is 0.
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }

    /// Uniform in [lo, hi], both included.
    fn between(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            lo
        } else {
            lo + (self.next() % (hi - lo + 1) as u64) as i64
        }
    }

    fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        items.get(self.below(items.len()))
    }

    /// One of `items` by weight; none when every weight is 0.
    fn weighted<'a, T>(&mut self, items: &'a [(T, f64)]) -> Option<&'a T> {
        let total: f64 = items.iter().map(|(_, w)| w).sum();
        let mut at = self.unit() * total;
        for (item, w) in items {
            if at < *w {
                return Some(item);
            }
            at -= w;
        }
        items.last().map(|(item, _)| item)
    }

    /// `k` distinct indices below `n` (fewer when `n` is smaller), in order.
    fn sample(&mut self, n: usize, k: usize) -> Vec<usize> {
        let mut chosen = BTreeMap::new();
        let k = k.min(n);
        while chosen.len() < k {
            chosen.insert(self.below(n), ());
        }
        chosen.into_keys().collect()
    }
}

/// Phone numbers, each handed out once: a mobile prefix and seven digits
/// spread by a multiplier coprime to 10^7, so neighbours do not look alike.
struct PhoneBook {
    prefixes: Vec<String>,
    next: u64,
}

impl PhoneBook {
    fn take(&mut self) -> String {
        let count = self.prefixes.len().max(1) as u64;
        let prefix = self
            .prefixes
            .get((self.next % count) as usize)
            .map_or("090", String::as_str);
        let body = ((self.next / count) * 7_919 + 1_234_567) % 10_000_000;
        self.next += 1;
        format!("{prefix}{body:07}")
    }
}

/// `text` with every `{key}` replaced by its value.
fn fill(text: &str, values: &[(&str, &str)]) -> String {
    values.iter().fold(text.to_owned(), |t, (key, value)| {
        t.replace(&format!("{{{key}}}"), value)
    })
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
}

fn upper_first(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// A price rounded the way a market quotes it.
fn round_price(price: f64, top: i64) -> i64 {
    let step = match top {
        t if t >= 1_000_000 => 10_000.0,
        t if t >= 100_000 => 1_000.0,
        t if t >= 10_000 => 100.0,
        _ => 10.0,
    };
    ((price / step).round() * step) as i64
}

struct Person {
    full: String,
    /// "Tư Hùng": how farmers call each other.
    called: String,
    rank: String,
    given: String,
}

/// A generated shop, by the facts the activity around it is drawn from.
struct Shop {
    name: String,
    ward: usize,
    sector: Option<String>,
    services: Vec<String>,
    products: Vec<String>,
    placed: bool,
    /// How busy its ward is, 0 to 1.
    busy: f64,
}

/// A generated area preset: its label and its ward.
struct Preset {
    label: String,
    ward: usize,
}

struct Generator<'a> {
    data: DemoData,
    vocab: &'a Vocabulary,
    wards: &'a [WardPlace],
    rng: Rng,
    phones: PhoneBook,
    providers: usize,
    /// Each ward's share of the shops.
    ward_weights: Vec<(usize, f64)>,
    /// Each ward's share against the busiest one, 0 to 1.
    busy: Vec<f64>,
    shops: Vec<Shop>,
    presets: Vec<Preset>,
    farmers: Vec<Person>,
    farmer_phones: Vec<String>,
}

/// Generates the demo data at volume on top of `base` (the file's).
pub(crate) fn generate(
    base: DemoData,
    vocab: &Vocabulary,
    wards: &[WardPlace],
    volume: &DemoVolume,
) -> DemoData {
    let mut g = Generator {
        data: base,
        vocab,
        wards,
        rng: Rng(volume.seed),
        phones: PhoneBook {
            prefixes: vocab.phone_prefixes.clone(),
            next: 0,
        },
        providers: volume.providers as usize,
        ward_weights: Vec::new(),
        busy: Vec::new(),
        shops: Vec::new(),
        presets: Vec::new(),
        farmers: Vec::new(),
        farmer_phones: Vec::new(),
    };
    g.catalogue();
    g.weigh_wards();
    g.area_presets();
    g.people();
    g.listings();
    g.taps();
    g.requests_and_reviews();
    g.searches();
    g.prices();
    g.jobs();
    g.partners();
    g.alerts();
    g.data
}

impl Generator<'_> {
    /// The vocabulary's catalogue on top of the file's, nothing twice.
    fn catalogue(&mut self) {
        let v = self.vocab;
        let d = &mut self.data;
        for s in &v.sectors {
            if !d.sectors.iter().any(|x| x.code == s.code) {
                d.sectors.push(s.clone());
            }
        }
        for k in &v.alert_kinds {
            if !d.alert_kinds.iter().any(|x| x.code == k.code) {
                d.alert_kinds.push(k.clone());
            }
        }
        for c in &v.commodities {
            if !d.commodities.iter().any(|x| x.name == c.name) {
                d.commodities.push(c.clone());
            }
        }
        for t in &v.service_types {
            if !d.service_types.iter().any(|x| x.slug == t.slug) {
                d.service_types.push(t.clone());
            }
        }
        for t in &v.supply_types {
            if !d.supply_types.iter().any(|x| x.slug == t.slug) {
                d.supply_types.push(t.clone());
            }
        }
        let mut order = d.crops.iter().map(|c| c.display_order).max().unwrap_or(0);
        for family in &v.crops {
            for name in &family.names {
                if !d.crops.iter().any(|c| &c.name == name) {
                    order += 1;
                    d.crops.push(DemoCrop {
                        name: name.clone(),
                        commodity: family.commodity.clone(),
                        unit: family.unit.clone(),
                        display_order: order,
                    });
                }
            }
        }
    }

    /// Each province its share of the shops, spread over its wards so a few
    /// are busy and most are quiet, as real demand is.
    fn weigh_wards(&mut self) {
        self.ward_weights = Vec::new();
        for share in &self.vocab.provinces {
            let mine: Vec<usize> = self
                .wards
                .iter()
                .enumerate()
                .filter(|(_, w)| w.province_code == share.code)
                .map(|(i, _)| i)
                .collect();
            // Which ward is busiest is drawn, not the first in code order.
            let mut keys: Vec<(u64, usize)> = mine.iter().map(|w| (self.rng.next(), *w)).collect();
            keys.sort_unstable();
            let shuffled: Vec<usize> = keys.into_iter().map(|(_, w)| w).collect();
            let falloff: Vec<f64> = (0..shuffled.len())
                .map(|rank| 1.0 / ((rank + 1) as f64).powf(0.8))
                .collect();
            let total: f64 = falloff.iter().sum();
            for (ward, f) in shuffled.into_iter().zip(falloff) {
                self.ward_weights.push((ward, share.weight * f / total));
            }
        }
        let top = self
            .ward_weights
            .iter()
            .map(|(_, w)| *w)
            .fold(0.0_f64, f64::max);
        self.busy = vec![0.0; self.wards.len()];
        for (ward, w) in &self.ward_weights {
            if let Some(b) = self.busy.get_mut(*ward) {
                *b = if top > 0.0 { w / top } else { 0.0 };
            }
        }
    }

    fn ward(&self, i: usize) -> Option<&WardPlace> {
        self.wards.get(i)
    }

    fn busy(&self, ward: usize) -> f64 {
        self.busy.get(ward).copied().unwrap_or(0.0)
    }

    /// The busiest wards of each province as area presets ("Bình Phú, Đồng Tháp"),
    /// as the admin's ward picker names them.
    fn area_presets(&mut self) {
        let mut order = self
            .data
            .locations
            .iter()
            .map(|l| l.display_order)
            .max()
            .unwrap_or(0);
        for share in &self.vocab.provinces {
            let mut mine: Vec<(usize, f64)> = self
                .ward_weights
                .iter()
                .filter(|(w, _)| self.ward(*w).is_some_and(|p| p.province_code == share.code))
                .copied()
                .collect();
            mine.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            for (ward, _) in mine.into_iter().take(share.presets) {
                let Some(place) = self.wards.get(ward) else {
                    continue;
                };
                let label = preset_label(&place.province, &place.name);
                if !self.data.locations.iter().any(|l| l.label == label) {
                    order += 1;
                    self.data.locations.push(DemoLocation {
                        label: label.clone(),
                        lat: place.lat,
                        lng: place.lng,
                        display_order: order,
                    });
                }
                self.presets.push(Preset { label, ward });
            }
        }
    }

    fn person(&mut self) -> Person {
        let p = &self.vocab.people;
        let female = self.rng.chance(0.4);
        let (middles, givens) = if female {
            (&p.middle_female, &p.given_female)
        } else {
            (&p.middle_male, &p.given_male)
        };
        let family = self.rng.pick(&p.family).cloned().unwrap_or_default();
        let middle = self.rng.pick(middles).cloned().unwrap_or_default();
        let given = self.rng.pick(givens).cloned().unwrap_or_default();
        let rank = self.rng.pick(&p.rank).cloned().unwrap_or_default();
        Person {
            full: format!("{family} {middle} {given}"),
            called: format!("{rank} {given}"),
            rank,
            given,
        }
    }

    /// The farmers who send requests, post jobs and write reviews.
    fn people(&mut self) {
        for _ in 0..self.providers * FARMERS_PER_PROVIDER {
            let person = self.person();
            self.farmers.push(person);
            let phone = self.phones.take();
            self.farmer_phones.push(phone);
        }
    }

    /// A farmer: how they sign a request, and their phone.
    fn farmer(&mut self) -> (String, String) {
        let i = self.rng.below(self.farmers.len());
        let casual = self.rng.chance(0.5);
        let name = self
            .farmers
            .get(i)
            .map(|p| {
                if casual {
                    p.called.clone()
                } else {
                    p.full.clone()
                }
            })
            .unwrap_or_default();
        let phone = self.farmer_phones.get(i).cloned().unwrap_or_default();
        (name, phone)
    }

    /// The sector a shop works in: mostly crops, as in the Delta.
    fn focus(&mut self) -> Option<String> {
        let shares: Vec<(Option<&str>, f64)> = vec![
            (Some("crop"), 60.0),
            (Some("livestock"), 15.0),
            (Some("aquatic"), 15.0),
            (Some("lam-nghiep"), 3.0),
            (Some("nam-duoc-lieu"), 3.0),
            (Some("diem-nghiep"), 1.0),
            (None, 3.0),
        ];
        let known: Vec<(Option<&str>, f64)> = shares
            .into_iter()
            .filter(|(s, _)| s.is_none_or(|s| self.data.sectors.iter().any(|x| x.code == s)))
            .collect();
        self.rng
            .weighted(&known)
            .copied()
            .flatten()
            .map(str::to_owned)
    }

    /// Types serving `sector` (theirs, or every sector's) that have words.
    fn types_for<'t, T>(
        types: &'t [DemoType],
        words: &BTreeMap<String, Vec<T>>,
        sector: Option<&str>,
    ) -> Vec<&'t DemoType> {
        types
            .iter()
            .filter(|t| words.get(&t.slug).is_some_and(|w| !w.is_empty()))
            .filter(|t| t.sector.is_none() || t.sector.as_deref() == sector)
            .collect()
    }

    fn unique_name(taken: &HashSet<String>, name: String, ward: &str) -> String {
        if !taken.contains(&name) {
            return name;
        }
        let with_ward = format!("{name} {ward}");
        if !taken.contains(&with_ward) {
            return with_ward;
        }
        (2..)
            .map(|n| format!("{with_ward} {n}"))
            .find(|candidate| !taken.contains(candidate))
            .unwrap_or(with_ward)
    }

    /// The shops, their accounts, services and supplies.
    fn listings(&mut self) {
        let mut taken: HashSet<String> = self
            .data
            .businesses
            .iter()
            .map(|b| b.name.clone())
            .collect();
        let weights = self.ward_weights.clone();
        for i in 0..self.providers {
            let Some(&ward) = self.rng.weighted(&weights) else {
                return;
            };
            let Some(place) = self.wards.get(ward).cloned() else {
                continue;
            };
            let kind = *self
                .rng
                .weighted(&[("service", 55.0), ("dealer", 30.0), ("both", 15.0)])
                .unwrap_or(&"service");
            let sector = self.focus();
            let owner = self.person();

            let mut services: Vec<DemoService> = Vec::new();
            let mut first_service = String::new();
            if kind != "dealer" {
                let types = Self::types_for(
                    &self.data.service_types,
                    &self.vocab.service_offers,
                    sector.as_deref(),
                );
                let count = *self
                    .rng
                    .weighted(&[(1usize, 60.0), (2, 30.0), (3, 10.0)])
                    .unwrap_or(&1);
                for t in self.rng.sample(types.len(), count) {
                    let Some(t) = types.get(t) else { continue };
                    let offers = self.vocab.service_offers.get(&t.slug);
                    let Some(offer) = offers.and_then(|o| self.rng.pick(o)) else {
                        continue;
                    };
                    if services.iter().any(|s| s.name == offer.name) {
                        continue;
                    }
                    if first_service.is_empty() {
                        first_service = lower_first(&t.name);
                    }
                    services.push(DemoService {
                        business: String::new(),
                        category_slug: t.slug.clone(),
                        name: offer.name.clone(),
                        description: offer.description.clone(),
                        price_note: offer.price_note.clone(),
                    });
                }
            }
            let mut products: Vec<DemoProduct> = Vec::new();
            let mut first_supply = String::new();
            if kind != "service" {
                let types = Self::types_for(
                    &self.data.supply_types,
                    &self.vocab.supply_items,
                    sector.as_deref(),
                );
                let count = self.rng.between(1, 5) as usize;
                let mut names = HashSet::new();
                for _ in 0..count {
                    let Some(t) = self.rng.pick(&types).copied() else {
                        break;
                    };
                    let items = self.vocab.supply_items.get(&t.slug);
                    let Some(item) = items.and_then(|i| self.rng.pick(i)) else {
                        continue;
                    };
                    if !names.insert(item.name.clone()) {
                        continue;
                    }
                    if first_supply.is_empty() {
                        first_supply = lower_first(&t.name);
                    }
                    let (lo, hi) = item.price;
                    let price = round_price(self.rng.between(lo, hi) as f64, hi).max(lo.max(1));
                    products.push(DemoProduct {
                        business: String::new(),
                        category_slug: t.slug.clone(),
                        commodity: item.commodity.clone(),
                        name: item.name.clone(),
                        description: format!("{}, {}", item.name, item.unit),
                        price: price.min(PRICE_PER_UNIT_MAX),
                        unit: item.unit.clone(),
                    });
                }
            }
            if services.is_empty() && products.is_empty() {
                continue;
            }

            let patterns = match kind {
                "dealer" => &self.vocab.shop_names.dealer,
                "both" => &self.vocab.shop_names.both,
                _ => &self.vocab.shop_names.service,
            };
            let pattern = self.rng.pick(patterns).cloned().unwrap_or_default();
            let named = fill(
                &pattern,
                &[
                    ("person", &owner.full),
                    ("rank", &owner.rank),
                    ("given", &owner.given),
                    ("ward", &place.name),
                    ("service", &first_service),
                    ("supply", &first_supply),
                ],
            );
            let name = Self::unique_name(&taken, named, &place.name);
            taken.insert(name.clone());

            let offers: Vec<String> = services
                .iter()
                .map(|s| s.name.clone())
                .chain(products.iter().map(|p| p.name.clone()))
                .take(3)
                .map(|n| lower_first(&n))
                .collect();
            let template = self
                .rng
                .pick(&self.vocab.shop_descriptions)
                .cloned()
                .unwrap_or_default();
            let years = self.rng.between(2, 25).to_string();
            let description = upper_first(&fill(
                &template,
                &[
                    ("offers", &offers.join(", ")),
                    ("ward", &place.name),
                    ("years", &years),
                ],
            ));
            let placed = !self.rng.chance(0.03);
            let jitter = |r: &mut Rng| (r.unit() * 2.0 - 1.0) * SHOP_JITTER_DEG;
            let (lat, lng) = if placed {
                (
                    Some(place.lat + jitter(&mut self.rng)),
                    Some(place.lng + jitter(&mut self.rng)),
                )
            } else {
                (None, None)
            };
            let account = self.rng.chance(0.96).then(|| {
                let mut local: String = slugify(&owner.full).replace('-', "");
                let suffix = (i + 1).to_string();
                local.truncate(EMAIL_LOCAL_MAX.saturating_sub(suffix.len()));
                local.push_str(&suffix);
                DemoAccount {
                    email: format!("{local}@{}", self.vocab.email_domain),
                    full_name: owner.full.clone(),
                    verified: self.rng.chance(0.95),
                }
            });
            let phone = self.phones.take();
            let is_verified = self.rng.chance(0.35);
            self.data.businesses.push(DemoBusiness {
                name: name.clone(),
                phone,
                description,
                address: format!("{}, {}", place.full_name, place.province),
                lat,
                lng,
                is_verified,
                account,
            });
            let service_names: Vec<String> = services.iter().map(|s| s.name.clone()).collect();
            let product_names: Vec<String> = products.iter().map(|p| p.name.clone()).collect();
            for mut s in services {
                s.business = name.clone();
                self.data.services.push(s);
            }
            for mut p in products {
                p.business = name.clone();
                self.data.products.push(p);
            }
            let busy = self.busy(ward);
            self.shops.push(Shop {
                name,
                ward,
                sector,
                services: service_names,
                products: product_names,
                placed,
                busy,
            });
        }
    }

    /// A shop, busier ones more often.
    fn busy_shop(&mut self) -> Option<usize> {
        if self.shops.is_empty() {
            return None;
        }
        for _ in 0..32 {
            let i = self.rng.below(self.shops.len());
            let busy = self.shops.get(i).map_or(0.0, |s| s.busy);
            if self.rng.chance(0.25 + 0.75 * busy) {
                return Some(i);
            }
        }
        Some(self.rng.below(self.shops.len()))
    }

    /// `DEMAND_WEEKS` weekly counts around `rate`, growing a little.
    fn weekly(&mut self, rate: f64) -> Vec<u32> {
        let weeks = DEMAND_WEEKS.max(1);
        (0..weeks)
            .map(|w| {
                let trend = 0.8 + 0.4 * w as f64 / (weeks.max(2) - 1) as f64;
                let noise = 0.6 + 0.8 * self.rng.unit();
                (rate * trend * noise).round().max(0.0) as u32
            })
            .collect()
    }

    /// One of a shop's services or supplies: (service, product), at most one.
    fn item_of(&mut self, shop: usize) -> Option<(Option<String>, Option<String>)> {
        let s = self.shops.get(shop)?;
        let all = s.services.len() + s.products.len();
        let i = self.rng.below(all);
        Some(match s.services.get(i) {
            Some(name) => (Some(name.clone()), None),
            None => (None, s.products.get(i - s.services.len()).cloned()),
        })
    }

    /// Call and Zalo taps, by report week.
    fn taps(&mut self) {
        for shop in 0..self.shops.len() {
            let Some(s) = self.shops.get(shop) else {
                continue;
            };
            let (name, busy) = (s.name.clone(), s.busy);
            let rate = 0.3 + 4.5 * busy.powf(0.7);
            for (channel, share) in [("call", 1.0), ("zalo", 0.35)] {
                let weekly = self.weekly(rate * share);
                if weekly.iter().all(|n| *n == 0) {
                    continue;
                }
                let (service, product) = if self.rng.chance(0.6) {
                    self.item_of(shop).unwrap_or((None, None))
                } else {
                    (None, None)
                };
                self.data.activity.calls.push(DemoCalls {
                    business: name.clone(),
                    service,
                    product,
                    channel: channel.to_owned(),
                    weekly,
                });
            }
        }
    }

    /// Contact requests, then reviews from the phones of the answered ones.
    fn requests_and_reviews(&mut self) {
        let mut asked: HashSet<(usize, String)> = HashSet::new();
        let mut answered: Vec<(usize, String, String, i64)> = Vec::new();
        for _ in 0..self.providers * INQUIRIES_PER_PROVIDER {
            let Some(shop) = self.busy_shop() else { return };
            let (name, phone) = self.farmer();
            if !asked.insert((shop, phone.clone())) {
                continue;
            }
            let Some((service, product)) = self.item_of(shop) else {
                continue;
            };
            let topic = service
                .clone()
                .or_else(|| product.clone())
                .unwrap_or_default();
            let days_ago = (self.rng.unit().powf(1.5) * INQUIRY_DAYS_BACK) as i64;
            let status = if days_ago <= 2 {
                *self
                    .rng
                    .weighted(&[("new", 75.0), ("contacted", 25.0)])
                    .unwrap_or(&"new")
            } else {
                *self
                    .rng
                    .weighted(&[
                        ("new", 8.0),
                        ("contacted", 35.0),
                        ("closed", 47.0),
                        ("spam", 10.0),
                    ])
                    .unwrap_or(&"closed")
            };
            let ward = self
                .shops
                .get(shop)
                .and_then(|s| self.wards.get(s.ward))
                .map(|w| w.name.clone())
                .unwrap_or_default();
            let template = self
                .rng
                .pick(&self.vocab.inquiry_notes)
                .cloned()
                .unwrap_or_default();
            let size = self
                .rng
                .pick(&self.vocab.farm_sizes)
                .cloned()
                .unwrap_or_default();
            let when = self.rng.pick(&self.vocab.when).cloned().unwrap_or_default();
            let note = upper_first(&fill(
                &template,
                &[
                    ("item", &lower_first(&topic)),
                    ("size", &size),
                    ("when", &when),
                    ("ward", &ward),
                ],
            ));
            let business = self
                .shops
                .get(shop)
                .map(|s| s.name.clone())
                .unwrap_or_default();
            if status == "contacted" || status == "closed" {
                answered.push((shop, phone.clone(), name.clone(), days_ago));
            }
            self.data.activity.inquiries.push(DemoInquiry {
                business,
                service,
                product,
                topic,
                name,
                phone,
                note,
                status: status.to_owned(),
                days_ago,
            });
        }
        let share = if answered.is_empty() {
            0.0
        } else {
            (self.providers as f64 / answered.len() as f64).min(1.0)
        };
        for (shop, phone, name, asked_days_ago) in answered {
            if !self.rng.chance(share) {
                continue;
            }
            let rating = *self
                .rng
                .weighted(&[(5i16, 45.0), (4, 30.0), (3, 12.0), (2, 7.0), (1, 6.0)])
                .unwrap_or(&5);
            let words = match rating {
                4 | 5 => &self.vocab.reviews.good,
                3 => &self.vocab.reviews.mixed,
                _ => &self.vocab.reviews.bad,
            };
            let comment = self.rng.pick(words).cloned().unwrap_or_default();
            let visible = self.rng.chance(0.72);
            let reply = if visible && self.rng.chance(0.4) {
                self.rng.pick(&self.vocab.replies).cloned()
            } else {
                None
            };
            // After the request, and a day before today at the latest so
            // the reply can follow it.
            let back = self.rng.between(0, asked_days_ago.clamp(0, 5));
            let days_ago = (asked_days_ago - back).max(1);
            let business = self
                .shops
                .get(shop)
                .map(|s| s.name.clone())
                .unwrap_or_default();
            self.data.activity.reviews.push(DemoReview {
                business,
                phone,
                name,
                rating,
                comment,
                reply,
                visible,
                days_ago,
            });
        }
    }

    fn type_slugs(&self) -> Vec<String> {
        self.data
            .service_types
            .iter()
            .chain(&self.data.supply_types)
            .map(|t| t.slug.clone())
            .collect()
    }

    /// Searches near each generated preset, by report week; some find
    /// nothing, for the report's unmet searches.
    fn searches(&mut self) {
        let slugs = self.type_slugs();
        for p in 0..self.presets.len() {
            let Some(preset) = self.presets.get(p) else {
                continue;
            };
            let (label, busy) = (preset.label.clone(), self.busy(preset.ward));
            for _ in 0..2 {
                let Some(slug) = self.rng.pick(&slugs).cloned() else {
                    break;
                };
                let weekly = self.weekly(1.0 + 6.0 * busy);
                let results = self.rng.between(1, 15) as i32;
                self.data.activity.searches.push(DemoSearches {
                    query: slug.replace('-', " "),
                    category_slug: Some(slug),
                    location: label.clone(),
                    results,
                    weekly,
                });
            }
            if self.rng.chance(0.3) {
                let Some(query) = self.rng.pick(&self.vocab.search_queries_unmet).cloned() else {
                    continue;
                };
                let weekly = self.weekly(0.5 + 2.0 * busy);
                self.data.activity.searches.push(DemoSearches {
                    query,
                    category_slug: None,
                    location: label,
                    results: 0,
                    weekly,
                });
            }
        }
    }

    /// Farm-gate prices of every crop in several presets, a few days each,
    /// moving a little from one to the next.
    fn prices(&mut self) {
        let ranges: HashMap<&str, (i64, i64)> = self
            .vocab
            .crops
            .iter()
            .map(|f| (f.commodity.as_str(), f.price))
            .collect();
        let used: HashSet<(String, String)> = self
            .data
            .activity
            .prices
            .iter()
            .map(|p| (p.crop.clone(), p.location.clone()))
            .collect();
        let crops: Vec<(String, (i64, i64))> = self
            .data
            .crops
            .iter()
            .filter_map(|c| Some((c.name.clone(), *ranges.get(c.commodity.as_str())?)))
            .collect();
        for (crop, (lo, hi)) in crops {
            let areas = self.rng.between(PRICE_AREAS.0, PRICE_AREAS.1) as usize;
            for p in self.rng.sample(self.presets.len(), areas) {
                let Some(label) = self.presets.get(p).map(|p| p.label.clone()) else {
                    continue;
                };
                if used.contains(&(crop.clone(), label.clone())) {
                    continue;
                }
                let days = self.rng.between(3, 6) as usize;
                let mut back: Vec<i64> = self
                    .rng
                    .sample(PRICE_DAYS_BACK as usize + 1, days)
                    .into_iter()
                    .map(|d| d as i64)
                    .collect();
                back.reverse(); // oldest first, so the walk runs forward in time
                let mut mid = lo as f64 + self.rng.unit() * (hi - lo) as f64;
                let mut points = Vec::new();
                for days_ago in back {
                    mid =
                        (mid * (1.0 + (self.rng.unit() - 0.5) * 0.12)).clamp(lo as f64, hi as f64);
                    let spread = mid * (0.02 + 0.05 * self.rng.unit());
                    let low = round_price(mid - spread, hi).max(1);
                    if low > PRICE_PER_UNIT_MAX {
                        continue;
                    }
                    let high = round_price(mid + spread, hi).clamp(low, PRICE_PER_UNIT_MAX);
                    points.push((days_ago, low, high));
                }
                if !points.is_empty() {
                    self.data.activity.prices.push(DemoPrices {
                        crop: crop.clone(),
                        location: label,
                        points,
                    });
                }
            }
        }
    }

    /// A generated preset, busier ones more often.
    fn busy_preset(&mut self) -> Option<usize> {
        if self.presets.is_empty() {
            return None;
        }
        for _ in 0..32 {
            let i = self.rng.below(self.presets.len());
            let busy = self.presets.get(i).map_or(0.0, |p| self.busy(p.ward));
            if self.rng.chance(0.25 + 0.75 * busy) {
                return Some(i);
            }
        }
        Some(self.rng.below(self.presets.len()))
    }

    /// Job posts: a few open this week, most closed months ago.
    fn jobs(&mut self) {
        let types: Vec<(String, String)> = self
            .data
            .service_types
            .iter()
            .map(|t| (t.slug.clone(), lower_first(&t.name)))
            .collect();
        for _ in 0..self.providers {
            let Some(p) = self.busy_preset() else { return };
            let Some((label, ward)) = self.presets.get(p).map(|p| {
                (
                    p.label.clone(),
                    self.wards
                        .get(p.ward)
                        .map(|w| w.name.clone())
                        .unwrap_or_default(),
                )
            }) else {
                continue;
            };
            let Some((slug, service)) = self.rng.pick(&types).cloned() else {
                return;
            };
            let days_ago = if self.rng.chance(0.3) {
                self.rng.between(0, 7)
            } else {
                self.rng.between(8, JOB_DAYS_BACK)
            };
            let status = if days_ago <= 7 {
                *self
                    .rng
                    .weighted(&[("open", 75.0), ("closed", 15.0), ("spam", 10.0)])
                    .unwrap_or(&"open")
            } else {
                *self
                    .rng
                    .weighted(&[("closed", 82.0), ("open", 8.0), ("spam", 10.0)])
                    .unwrap_or(&"closed")
            };
            let template = self
                .rng
                .pick(&self.vocab.job_descriptions)
                .cloned()
                .unwrap_or_default();
            let size = self
                .rng
                .pick(&self.vocab.farm_sizes)
                .cloned()
                .unwrap_or_default();
            let when = self.rng.pick(&self.vocab.when).cloned().unwrap_or_default();
            let description = upper_first(&fill(
                &template,
                &[
                    ("service", &service),
                    ("size", &size),
                    ("when", &when),
                    ("ward", &ward),
                ],
            ));
            let (name, phone) = self.farmer();
            self.data.activity.jobs.push(DemoJob {
                category_slug: slug,
                location: label,
                days_ago,
                description,
                name,
                phone,
                status: status.to_owned(),
            });
        }
    }

    /// Partners with contracts over the ward or province of a shop, and
    /// shops of that area they sponsor.
    fn partners(&mut self) {
        let count = (self.providers / PROVIDERS_PER_PARTNER).max(4);
        let services: Vec<String> = self
            .data
            .service_types
            .iter()
            .map(|t| t.slug.clone())
            .collect();
        let general: Vec<String> = self
            .data
            .service_types
            .iter()
            .filter(|t| t.sector.is_none())
            .map(|t| t.slug.clone())
            .collect();
        let supplies: Vec<String> = self
            .data
            .supply_types
            .iter()
            .map(|t| t.slug.clone())
            .collect();
        let mut by_ward: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut by_province: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, s) in self.shops.iter().enumerate() {
            if !s.placed || s.services.is_empty() {
                continue;
            }
            by_ward.entry(s.ward).or_default().push(i);
            if let Some(w) = self.wards.get(s.ward) {
                by_province
                    .entry(w.province_code.clone())
                    .or_default()
                    .push(i);
            }
        }
        let mut names: HashSet<String> = self
            .data
            .activity
            .partners
            .iter()
            .map(|p| p.name.clone())
            .collect();
        let vocab = self.vocab;
        let words = &vocab.partners;
        for k in 1..=count {
            let kind = *self
                .rng
                .weighted(&[
                    ("brand", 35.0),
                    ("dealer", 35.0),
                    ("cooperative", 20.0),
                    ("exporter", 10.0),
                ])
                .unwrap_or(&"brand");
            let (patterns, categories): (&Vec<String>, Vec<String>) = match kind {
                "dealer" => (&words.dealer, [supplies.clone(), services.clone()].concat()),
                "cooperative" => (&words.cooperative, services.clone()),
                "exporter" => (&words.exporter, general.clone()),
                _ => (&words.brand, supplies.clone()),
            };
            let pattern = self.rng.pick(patterns).cloned().unwrap_or_default();
            let place = self.rng.pick(&words.names).cloned().unwrap_or_default();
            let named = fill(&pattern, &[("name", &place)]);
            let name = Self::unique_name(&names, named, &k.to_string());
            names.insert(name.clone());
            let contact = self.person();
            let account = self.rng.chance(0.8).then(|| DemoAccount {
                email: format!("doitac{k}@{}", self.vocab.email_domain),
                full_name: contact.full.clone(),
                verified: true,
            });
            let mut contracts = Vec::new();
            for _ in 0..self.rng.between(1, 4) {
                let Some(anchor) = self.busy_shop() else {
                    break;
                };
                let Some(shop) = self.shops.get(anchor) else {
                    continue;
                };
                if !shop.placed {
                    continue;
                }
                let (shop_name, ward) = (shop.name.clone(), shop.ward);
                let whole_province = self.rng.chance(0.45);
                let pool: Vec<usize> = if whole_province {
                    self.wards
                        .get(ward)
                        .and_then(|w| by_province.get(&w.province_code))
                        .cloned()
                        .unwrap_or_default()
                } else {
                    by_ward.get(&ward).cloned().unwrap_or_default()
                };
                let picked = self.rng.between(0, 3) as usize;
                let sponsored: Vec<String> = self
                    .rng
                    .sample(pool.len(), picked)
                    .into_iter()
                    .filter_map(|i| pool.get(i))
                    .filter_map(|s| self.shops.get(*s).map(|s| s.name.clone()))
                    .collect();
                let how_many = self.rng.between(1, 3) as usize;
                let chosen: Vec<String> = self
                    .rng
                    .sample(categories.len(), how_many)
                    .into_iter()
                    .filter_map(|i| categories.get(i).cloned())
                    .collect();
                let starts_in_days = self.rng.between(-300, 30);
                let ends_in_days = starts_in_days + self.rng.between(90, 365);
                let monthly_fee =
                    self.rng.between(2, 15) * 1_000_000 + self.rng.between(0, 1) * 500_000;
                let note = self.rng.pick(&words.notes).cloned().unwrap_or_default();
                contracts.push(DemoContract {
                    area: if whole_province {
                        DemoArea::ProvinceOf(shop_name)
                    } else {
                        DemoArea::WardOf(shop_name)
                    },
                    categories: chosen,
                    starts_in_days,
                    ends_in_days,
                    monthly_fee,
                    note,
                    sponsored,
                });
            }
            let phone = self.phones.take();
            self.data.activity.partners.push(DemoPartner {
                name,
                kind: kind.to_owned(),
                contact_name: contact.full,
                phone,
                email: format!("lienhe{k}@{}", self.vocab.email_domain),
                contracts,
                account,
            });
        }
    }

    /// Alerts over a shop's province: a few in force today, the rest past.
    fn alerts(&mut self) {
        let kinds: Vec<String> = self
            .data
            .alert_kinds
            .iter()
            .map(|k| k.code.clone())
            .filter(|code| self.vocab.alerts.get(code).is_some_and(|w| !w.is_empty()))
            .collect();
        for _ in 0..self.providers {
            let Some(anchor) = self.busy_shop() else {
                return;
            };
            let Some(shop) = self.shops.get(anchor) else {
                continue;
            };
            if !shop.placed {
                continue;
            }
            let (shop_name, shop_sector) = (shop.name.clone(), shop.sector.clone());
            let province = self
                .wards
                .get(shop.ward)
                .map(|w| w.province.clone())
                .unwrap_or_default();
            let Some(kind) = self.rng.pick(&kinds).cloned() else {
                return;
            };
            let Some(words) = self.vocab.alerts.get(&kind).and_then(|w| self.rng.pick(w)) else {
                continue;
            };
            let sector = if self.rng.chance(0.3) {
                None
            } else {
                shop_sector
            };
            let (starts_in_days, ends_in_days) = if self.rng.chance(0.005) {
                (self.rng.between(-5, 0), self.rng.between(1, 20))
            } else {
                let start = -self.rng.between(20, ALERT_DAYS_BACK);
                (start, (start + self.rng.between(3, 30)).min(-1))
            };
            self.data.activity.alerts.push(DemoAlert {
                kind,
                sector,
                province_of: Some(shop_name),
                title: fill(&words.title, &[("province", &province)]),
                body: fill(&words.body, &[("province", &province)]),
                starts_in_days,
                ends_in_days,
            });
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    //! The generator's own properties; the rules every demo row keeps are
    //! checked on its output by the demo data repository's tests.

    use super::*;

    /// Wards in two provinces of the vocabulary, enough to spread over.
    pub(crate) fn sample_wards() -> Vec<WardPlace> {
        let mut wards = Vec::new();
        for (p, province) in [("82", "P82"), ("91", "P91")] {
            for i in 0..40 {
                wards.push(WardPlace {
                    province_code: p.to_owned(),
                    province: province.to_owned(),
                    name: format!("W{p}-{i}"),
                    full_name: format!("Ward W{p}-{i}"),
                    lat: 10.0 + f64::from(i) * 0.01,
                    lng: 105.5 + f64::from(i) * 0.01,
                });
            }
        }
        wards
    }

    pub(crate) fn sample(providers: u32, seed: u64) -> Result<DemoData> {
        let base = crate::demo_data_repository::demo_data()?;
        Ok(generate(
            base,
            &vocabulary()?,
            &sample_wards(),
            &DemoVolume { providers, seed },
        ))
    }

    #[test]
    fn the_same_seed_gives_the_same_rows() -> Result<()> {
        let (a, b, c) = (sample(150, 7)?, sample(150, 7)?, sample(150, 8)?);
        let names = |d: &DemoData| {
            d.businesses
                .iter()
                .map(|b| b.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&a), names(&b));
        assert_ne!(names(&a), names(&c));
        assert_eq!(a.activity.inquiries.len(), b.activity.inquiries.len());
        Ok(())
    }

    #[test]
    fn it_grows_every_kind_in_proportion() -> Result<()> {
        let n = 400;
        let base = crate::demo_data_repository::demo_data()?;
        let d = sample(n, 1)?;
        let grown = |big: usize, small: usize| big.saturating_sub(small);
        let shops = grown(d.businesses.len(), base.businesses.len());
        assert!(shops >= n as usize * 9 / 10, "{shops} shops");
        let accounts = d.businesses.iter().filter(|b| b.account.is_some()).count();
        assert!(accounts >= shops * 9 / 10, "{accounts} accounts");
        assert!(grown(d.services.len(), base.services.len()) >= shops);
        assert!(grown(d.products.len(), base.products.len()) >= shops / 2);
        let reviews = grown(d.activity.reviews.len(), base.activity.reviews.len());
        assert!(reviews >= n as usize * 7 / 10, "{reviews} reviews");
        assert_eq!(
            grown(d.activity.jobs.len(), base.activity.jobs.len()),
            n as usize
        );
        assert!(grown(d.activity.alerts.len(), base.activity.alerts.len()) > 0);
        assert!(grown(d.activity.partners.len(), base.activity.partners.len()) >= 16);
        let prices: usize = d.activity.prices.iter().map(|p| p.points.len()).sum();
        assert!(prices >= 2_000, "{prices} prices");
        // Emails are unique and valid, as an account stores them.
        let mut emails = HashSet::new();
        let accounts = d
            .businesses
            .iter()
            .filter_map(|b| b.account.as_ref())
            .chain(
                d.activity
                    .partners
                    .iter()
                    .filter_map(|p| p.account.as_ref()),
            );
        for a in accounts {
            assert!(emails.insert(a.email.clone()), "{}", a.email);
            assert_eq!(a.email, a.email.to_lowercase());
            application::validation::validate_email(&a.email)?;
        }
        Ok(())
    }

    #[test]
    fn phones_are_handed_out_once() {
        let mut book = PhoneBook {
            prefixes: vec!["090".into(), "091".into()],
            next: 0,
        };
        let phones: Vec<String> = (0..10_000).map(|_| book.take()).collect();
        let unique: HashSet<&String> = phones.iter().collect();
        assert_eq!(unique.len(), phones.len());
        assert!(phones
            .iter()
            .all(|p| application::validation::is_valid_vn_phone(p)));
    }
}
