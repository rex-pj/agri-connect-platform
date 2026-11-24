import React, { useEffect, useState } from "react";
import { Box, Button, Icon, Page, Text, useNavigate } from "zmp-ui";

import { api } from "../api";
import { PriceLine } from "../components/price-line";
import { dayMonth } from "../format";
import { rememberedArea } from "../remember";
import { T } from "../texts";

/**
 * The few newest prices, as the website's home: of the area remembered on
 * the price screen when it has some (the title then names it), else of
 * every area. The server picks them (GET /api/explore/prices/latest).
 */
async function latestPrices() {
  const area = rememberedArea();
  if (area) {
    const there = await api.latestPrices(area);
    if (there.quotes.length > 0) return { ...there, area: there.quotes[0].location_label };
  }
  return { ...(await api.latestPrices()), area: "" };
}

/** One alert's words: its title, what to do, where and until when. */
function AlertText({ alert }) {
  return (
    <span className="alert__text">
      <strong>{alert.title}</strong>
      <Text size="small">{alert.body}</Text>
      <Text size="xSmall" className="muted">
        {alert.province_name ? `${alert.province_name} · ` : ""}
        {T.alertUntil(dayMonth(alert.ends_on))}
      </Text>
    </span>
  );
}

/**
 * The alerts in force, as the website's home: the newest shows, the others
 * fold under it. The newest is itself the button that unfolds them, with a
 * small "+n" and a chevron at its right, so the fold costs no row of its own.
 */
function Alerts({ alerts }) {
  const [open, setOpen] = useState(false);
  if (alerts.length === 0) return null;
  const [newest, ...rest] = alerts;
  return (
    <section className="alerts" aria-label={T.alertsLabel}>
      {rest.length === 0 ? (
        <div className="alert" role="alert">
          <AlertText alert={newest} />
        </div>
      ) : (
        <button
          type="button"
          className="alert alert--toggle"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
        >
          <AlertText alert={newest} />
          <span className="alert__more">
            {!open && <span aria-hidden="true">{T.alertsMoreShort(rest.length)}</span>}
            <span className="sr-only">{T.alertsMore(rest.length)}</span>
            <Icon icon={open ? "zi-chevron-up" : "zi-chevron-down"} size={16} />
          </span>
        </button>
      )}
      {open &&
        rest.map((a) => (
          <div key={a.id} className="alert" role="alert">
            <AlertText alert={a} />
          </div>
        ))}
    </section>
  );
}

/**
 * The alerts in force, the three errands (near me, prices, a job), the service
 * categories, the newest prices; a part the server closed (GET
 * /api/explore/features) is left out.
 */
export default function HomePage() {
  const navigate = useNavigate();
  const [categories, setCategories] = useState([]);
  const [latest, setLatest] = useState(null);
  const [alerts, setAlerts] = useState([]);
  // Every part open until the server says otherwise; one it closed is not offered.
  const [open, setOpen] = useState({ near_me: true, prices: true, jobs: true });

  useEffect(() => {
    api.categories().then(setCategories).catch(() => setCategories([]));
    latestPrices().then(setLatest).catch(() => setLatest(null));
    api.alerts().then(setAlerts).catch(() => setAlerts([]));
    api.features().then(setOpen).catch(() => {});
  }, []);

  const quotes = latest?.quotes || [];

  return (
    <Page className="page">
      <Box className="hero">
        <Text.Title size="large">{T.homeTitle}</Text.Title>
        <Text size="small">{T.homeSub}</Text>
      </Box>

      <Alerts alerts={alerts} />

      <Box className="errands">
        {open.near_me && (
          <Button size="large" fullWidth prefixIcon={<Icon icon="zi-location-solid" />} onClick={() => navigate("/near")}>
            {T.nearMe}
          </Button>
        )}
        {open.prices && (
          <Button size="large" variant="secondary" fullWidth prefixIcon={<Icon icon="zi-poll" />} onClick={() => navigate("/prices")}>
            {T.prices}
          </Button>
        )}
        {open.jobs && (
          <Button size="large" variant="secondary" fullWidth prefixIcon={<Icon icon="zi-post" />} onClick={() => navigate("/job")}>
            {T.postJob}
          </Button>
        )}
      </Box>

      {open.near_me && categories.length > 0 && (
        <Box className="section">
          <Text.Header>{T.services}</Text.Header>
          <div className="tiles">
            {categories.map((c) => (
              <button key={c.id} type="button" className="tile" onClick={() => navigate("/near", { state: { categoryId: c.id } })}>
                {c.name}
              </button>
            ))}
          </div>
        </Box>
      )}

      {open.prices && quotes.length > 0 && (
        <Box className="section">
          <div className="section__head">
            <Text.Header>{latest.area ? T.pricesIn(latest.area) : T.latestPrices}</Text.Header>
            <button type="button" className="link" onClick={() => navigate("/prices")}>{T.seeAll}</button>
          </div>
          <Box className="card">
            {quotes.map((q) => (
              <PriceLine key={`${q.crop_id}-${q.location_id}`} quote={q} today={latest.today} showCrop showArea={!latest.area} />
            ))}
          </Box>
        </Box>
      )}
    </Page>
  );
}
