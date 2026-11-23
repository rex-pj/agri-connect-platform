//! The way from a public page to where its record is edited: the admin
//! panel's page of it for an admin who may open that, the owner's form for
//! the provider whose business it is, nothing for anyone else. Only a link:
//! the page it leads to checks access itself.

use application::{
    authz::Actor,
    catalogue::{PERM_LISTING_MODERATE, PERM_PROVIDER_READ, PERM_SERVICE_MANAGE},
};
use domain::Error;

use crate::{
    app_state::AppState,
    routes,
    shell::{Area, Link},
};

/// The public record a page shows.
#[derive(Debug, Clone, Copy)]
pub enum Managed {
    Provider,
    Service(i32),
    Product(i32),
}

fn link(route: &str, id: Option<i32>, icon: &'static str, label: String) -> Result<Link, Error> {
    let id = id.map(|i| i.to_string());
    let params: Vec<(&str, &str)> = id.iter().map(|i| ("id", i.as_str())).collect();
    Ok(Link {
        href: routes::url(route, &params)
            .map_err(|e| Error::internal(format!("manage links: {e}")))?,
        icon,
        label,
    })
}

/// The edit link of `what` (of the business `business_id`) for `actor`.
pub async fn manage_link(
    state: &AppState,
    actor: Option<&Actor>,
    business_id: i32,
    what: Managed,
) -> Result<Option<Link>, Error> {
    let Some(actor) = actor else {
        return Ok(None);
    };
    let locale = state.locale();
    match Area::of(Some(actor)) {
        Area::Admin => {
            let (route, id, perm) = match what {
                Managed::Provider => ("admin_provider", business_id, PERM_PROVIDER_READ),
                Managed::Service(id) => ("admin_service", id, PERM_SERVICE_MANAGE),
                Managed::Product(id) => ("admin_product", id, PERM_LISTING_MODERATE),
            };
            if !actor.can(perm) {
                return Ok(None);
            }
            let label = locale.text("manage.admin");
            link(route, Some(id), "fa-shield-halved", label).map(Some)
        }
        Area::Provider => {
            let own = state
                .usecases
                .provider_dashboard
                .own_business_id(actor)
                .await?;
            if own != Some(business_id) {
                return Ok(None);
            }
            let (route, id) = match what {
                Managed::Provider => ("provider_dashboard", None),
                Managed::Service(id) => ("provider_service_edit", Some(id)),
                Managed::Product(id) => ("provider_product_edit", Some(id)),
            };
            link(route, id, "fa-pen", locale.text("manage.own")).map(Some)
        }
        Area::Partner | Area::Visitor => Ok(None),
    }
}
