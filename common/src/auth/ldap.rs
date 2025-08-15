use std::{
    collections::HashSet,
    fmt::{Debug, Display},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use async_trait::async_trait;
use ldap3::{LdapConnAsync, LdapConnSettings, Scope, SearchEntry};
use rustls::{
    Certificate,
    ClientConfig,
    PrivateKey,
    RootCertStore, // pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
};
use rustls_pki_types::{CertificateDer, PrivatePkcs8KeyDer, pem::PemObject};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, instrument, warn};

use crate::{auth::AuthzProvider, config::ESConfig};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LdapConfig {
    // url in normal ldap form ldaps://host:port
    pub url: String,
    // cert used to verify server connection
    pub ca_cert: PathBuf,
    // attribute for uids, i.e. uid
    pub uid_attr: String,
    // attribute for group names, i.e. cn
    pub gid_attr: String,
    // ldap search base for groups
    pub group_base: String,
    // filter used to find users in base, i.e. (&(objectClass=posixGroup)),
    // which will be combined with the user attr
    pub group_filter: String,
    // attribute for group membership, i.e. memberUid
    pub group_member_attr: String,
    // method for authenticating to the server
    pub auth: LdapClientAuth,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum LdapClientAuth {
    // pem-encoded tls cert and key to communicate with ldap server
    X509Cert { key: PathBuf, cert: PathBuf },
    // gssapi with kerberos lib defaults
    GssApi { fqdn: String },
}

pub struct LdapAuthz {
    config: LdapConfig,
    settings: LdapConnSettings,
}

#[async_trait]
impl AuthzProvider for LdapAuthz {
    fn new(config: Arc<ESConfig>) -> Result<Self> {
        let config = config.ldap.clone().expect("ldap config not found");

        // set up tls for verifying ldaps server cert
        let ca_cert: Vec<CertificateDer> =
            CertificateDer::pem_file_iter(&config.ca_cert)?.collect::<Result<Vec<_>, _>>()?;

        // legacy hacks for older rustls
        let ca_cert: Vec<Certificate> = ca_cert
            .iter()
            .map(|cert| Certificate(cert.to_vec()))
            .collect();

        let mut ca_root_store = RootCertStore::empty();
        for c in ca_cert {
            ca_root_store.add(&c)?
        }

        let tls_config = ClientConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_safe_default_protocol_versions()?
            .with_root_certificates(ca_root_store);

        let tls_config = match config.clone().auth {
            LdapClientAuth::X509Cert { key, cert } => {
                let key: PrivatePkcs8KeyDer = PemObject::from_pem_file(key)?;

                let cert: Vec<CertificateDer> =
                    CertificateDer::pem_file_iter(cert)?.collect::<Result<Vec<_>, _>>()?;

                // legacy hacks for older rustls needed by ldap
                let key = PrivateKey(key.secret_pkcs8_der().to_vec());
                let cert = cert.iter().map(|cert| Certificate(cert.to_vec())).collect();

                tls_config.with_client_auth_cert(cert, key)?
            }
            LdapClientAuth::GssApi { .. } => tls_config.with_no_client_auth(),
        };

        let settings = LdapConnSettings::new()
            .set_config(Arc::new(tls_config))
            .set_conn_timeout(Duration::from_secs(10));

        Ok(LdapAuthz { config, settings })
    }

    #[instrument(skip(self))]
    async fn groups_for_user(&self, uid: String) -> Result<HashSet<String>> {
        debug!("searching ldap for groups");

        let mut groups = HashSet::<String>::new();

        let (conn, mut ldap) =
            LdapConnAsync::with_settings(self.settings.clone(), &self.config.url).await?;

        ldap3::drive!(conn);

        // TODO -- the gssapi bind uses blocking calls
        match &self.config.auth {
            LdapClientAuth::X509Cert { .. } => ldap.sasl_external_bind().await?,
            LdapClientAuth::GssApi { fqdn } => ldap.sasl_gssapi_bind(fqdn).await?,
        };

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

        let (conn, mut ldap) =
            LdapConnAsync::with_settings(self.settings.clone(), &self.config.url).await?;

        ldap3::drive!(conn);

        match &self.config.auth {
            LdapClientAuth::X509Cert { .. } => ldap.sasl_external_bind().await?,
            LdapClientAuth::GssApi { fqdn } => ldap.sasl_gssapi_bind(fqdn).await?,
        };

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

impl Debug for LdapAuthz {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LdapAuthz")
            .field("config", &self.config)
            .finish()
    }
}

impl Display for LdapAuthz {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ldap group authorization via {}", self.config.url)
    }
}
