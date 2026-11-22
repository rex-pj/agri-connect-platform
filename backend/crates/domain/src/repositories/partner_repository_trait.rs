use async_trait::async_trait;
use chrono::NaiveDate;

use crate::{
    error::Error,
    models::{
        admin_list::AdminFilter,
        demand::DemoScope,
        partner::{
            ActiveContract, AreaShop, Contract, ContractArea, ContractFee, NewContract, Partner,
            PartnerFields,
        },
    },
};

#[async_trait]
pub trait PartnerRepositoryTrait {
    /// A page of the partners, on ones first, then by name (`search`: the
    /// name, the contact's name, phone or email; `state`: `on` / `off`).
    async fn list(
        &self,
        filter: &AdminFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Partner>, Error>;

    /// How many partners `list` pages through.
    async fn count(&self, filter: &AdminFilter) -> Result<i64, Error>;

    async fn find(&self, id: i32) -> Result<Option<Partner>, Error>;

    /// The partner the account `user_id` reads the report of.
    async fn find_by_account(&self, user_id: i32) -> Result<Option<i32>, Error>;

    async fn create(&self, fields: &PartnerFields, actor_id: i32) -> Result<i32, Error>;

    /// False when there is no such partner.
    async fn update(&self, id: i32, fields: &PartnerFields, actor_id: i32) -> Result<bool, Error>;

    /// Turns a partner on or off; `Some((name, changed))`, or `None` when
    /// there is no such partner. An off partner's contracts are not in force.
    async fn set_active(
        &self,
        id: i32,
        active: bool,
        actor_id: i32,
    ) -> Result<Option<(String, bool)>, Error>;

    /// Makes `user_id` the partner's account (replacing the previous one).
    async fn assign_account(&self, id: i32, user_id: i32) -> Result<(), Error>;

    /// A partner's contracts, the newest start first.
    async fn contracts(&self, partner_id: i32) -> Result<Vec<Contract>, Error>;

    /// A page of a partner's contracts, the newest start first.
    async fn contracts_page(
        &self,
        partner_id: i32,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Contract>, Error>;

    /// How many contracts a partner has.
    async fn count_contracts(&self, partner_id: i32) -> Result<i64, Error>;

    /// Contracts in force on `day` of partners that are on, without what
    /// they cover: what the revenue figures read.
    async fn fees_in_force(&self, day: NaiveDate) -> Result<Vec<ContractFee>, Error>;

    /// A contract and its categories, in one transaction; its id.
    async fn create_contract(&self, contract: &NewContract, actor_id: i32) -> Result<i32, Error>;

    /// The contract `contract_id` of `partner_id`, or `None`.
    async fn find_contract(
        &self,
        partner_id: i32,
        contract_id: i32,
    ) -> Result<Option<Contract>, Error>;

    /// Replaces a contract's dates, fee, note and categories (not its area),
    /// in one transaction; false when the partner has no such contract.
    async fn update_contract(
        &self,
        partner_id: i32,
        contract_id: i32,
        contract: &NewContract,
    ) -> Result<bool, Error>;

    /// False when the partner has no such contract.
    async fn delete_contract(&self, partner_id: i32, contract_id: i32) -> Result<bool, Error>;

    /// Adds a sponsored provider to a contract; adding it twice changes
    /// nothing.
    async fn add_sponsored(&self, contract_id: i32, business_id: i32) -> Result<(), Error>;

    /// The shops a contract may sponsor: lying in `area` (by their ward),
    /// of `scope`, not deleted, not sponsored by `contract_id` yet, their
    /// name matching `search` when given; by name, at most `limit`. The
    /// database picks them: an area can hold thousands.
    async fn sponsor_candidates(
        &self,
        contract_id: i32,
        area: &ContractArea,
        scope: DemoScope,
        search: Option<&str>,
        limit: u64,
    ) -> Result<Vec<AreaShop>, Error>;

    /// The shop `business_id` when it lies in `area` and is of `scope`.
    async fn shop_in_area(
        &self,
        business_id: i32,
        area: &ContractArea,
        scope: DemoScope,
    ) -> Result<Option<AreaShop>, Error>;

    /// False when the provider was not sponsored by that contract.
    async fn remove_sponsored(&self, contract_id: i32, business_id: i32) -> Result<bool, Error>;

    /// Contracts in force on `day` (between their dates, both included) of
    /// partners that are on.
    async fn active_contracts(&self, day: NaiveDate) -> Result<Vec<ActiveContract>, Error>;
}
