use std::{collections::HashSet, fmt::Display, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use ldap3::{LdapConnAsync, Scope, SearchEntry};
use tracing::{debug, error, instrument, warn};

use crate::{
    auth::AuthzBackend,
    config::{ESConfig, LdapConfig},
};

#[derive(Debug)]
pub struct LdapAuthz {
    config: LdapConfig,
}

#[async_trait]
impl AuthzBackend for LdapAuthz {
    fn new(config: Arc<ESConfig>) -> Result<Self> {
        let config = config.ldap.clone().expect("ldap config not found");

        Ok(LdapAuthz { config })
    }

    #[instrument(skip(self))]
    async fn groups_for_user(&self, uid: String) -> Result<HashSet<String>> {
        debug!("searching ldap for groups");

        let mut groups = HashSet::<String>::new();

        let (conn, mut ldap) = LdapConnAsync::new(&self.config.url).await?;

        ldap3::drive!(conn);

        // query the ldap server for all group entries whose memeber attribute contains the uid in question
        let (res_entries, _res) = ldap
            .search(
                &self.config.group_base,
                Scope::Subtree,
                &format!(
                    "(&({}={}){})",
                    self.config.group_member_attr, uid, self.config.group_filter
                ),
                vec![self.config.gid_attr.clone()],
            )
            .await?
            .success()?;

        // for each returned entry, check its string attribute map for the gid attribute
        for entry in res_entries {
            let entry = SearchEntry::construct(entry);

            if let Some(gids) = entry.attrs.get(&self.config.gid_attr) {
                // this really should be unique if the ldap server is correctly configured,
                // but we should guard against it regardless
                match gids.len() {
                    1 => {
                        if let Some(gid) = gids.first() {
                            groups.insert(gid.clone());
                        } else {
                            error!(
                                "internal error: failed to extract ldap group attribute from map"
                            );
                        }
                    }
                    _ => {
                        warn!({gid_attr = self.config.gid_attr, gids = ?gids}, "ldap group object has incorrect number of gid attributes");
                    }
                }
            }
        }

        ldap.unbind().await?;

        debug!({groups = ?groups}, "found groups in ldap");

        Ok(groups)
    }

    #[instrument(skip(self))]
    async fn users_in_group(&self, gid: String) -> Result<HashSet<String>> {
        debug!("searching ldap for users");

        let mut users = HashSet::<String>::new();

        let (conn, mut ldap) = LdapConnAsync::new(&self.config.url).await?;

        ldap3::drive!(conn);

        // query the ldap server for all group entries whose memeber attribute contains the uid in question
        let (res_entries, _res) = ldap
            .search(
                &self.config.group_base,
                Scope::Subtree,
                &format!(
                    "(&({}={}){})",
                    self.config.gid_attr, gid, self.config.group_filter
                ),
                vec![self.config.group_member_attr.clone()],
            )
            .await?
            .success()?;

        // for each returned entry, check its string attribute map for the group_member attr
        for entry in res_entries {
            let entry = SearchEntry::construct(entry);

            if let Some(uids) = entry.attrs.get(&self.config.group_member_attr) {
                for uid in uids {
                    users.insert(uid.clone());
                }
            }
        }

        ldap.unbind().await?;

        debug!({users = ?users}, "found users in ldap");

        Ok(users)
    }
}

impl Display for LdapAuthz {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ldap group authorization via {}", self.config.url)
    }
}
