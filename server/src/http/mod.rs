use crate::service::EntanglementService;

pub mod auth;
pub mod msg;
pub mod svc;

pub trait HttpService: EntanglementService {}
