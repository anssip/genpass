use crate::graphql;
use crate::graphql::queries::MeQuery;
use crate::password::Credentials as CredentialsModel;

pub async fn grep(access_token: &str, master_password: &str, grep: &str) -> Vec<CredentialsModel> {
    let response = graphql::run_me_query(access_token, master_password, grep).await;
    let vaults = match response.data {
        Some(MeQuery { me }) => me.vaults,
        _ => {
            println!("No credentials found");
            Vec::new()
        }
    };
    let result = &mut Vec::new();
    for vault in vaults {
        if let Some(credentials) = vault.credentials {
            for creds in credentials {
                if let Some(cred) = creds {
                    result.push(CredentialsModel {
                        password: cred.password,
                        username: cred.username,
                        service: cred.service,
                    })
                }
            }
        }
    }
    result.to_vec()
}
