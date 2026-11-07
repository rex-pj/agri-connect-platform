use async_trait::async_trait;
use chrono::NaiveDate;

use crate::{
    error::Error,
    models::partner::{ActiveContract, Contract, NewContract, Partner, PartnerFields},
};

#[async_trait]
pub trait PartnerRepositoryTrait {
    /// Every partner, on ones first, then by name.
    async fn list(&self) -> Result<Vec<Partner>, Error>;

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

    /// False when the provider was not sponsored by that contract.
    async fn remove_sponsored(&self, contract_id: i32, business_id: i32) -> Result<bool, Error>;

    /// Contracts in force on `day` (between their dates, both included) of
    /// partners that are on.
    async fn active_contracts(&self, day: NaiveDate) -> Result<Vec<ActiveContract>, Error>;
}
