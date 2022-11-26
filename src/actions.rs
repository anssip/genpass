use crate::AccessTokens;
use crate::Credentials;
use async_trait::async_trait;
use clipboard::ClipboardContext;
use clipboard::ClipboardProvider;

use crate::auth;
use crate::keychain;
use crate::online_vault;
use crate::password;
use crate::store;
use crate::ui;
use anyhow::{bail, Context};
use clap::ArgMatches;
use log::{debug, info, warn};
use tokio::task;

pub async fn get_access_token() -> anyhow::Result<AccessTokens> {
    debug!("get_access_token()");
    if !store::has_logged_in() {
        bail!("You are not logged in to the Passlane Online Vault. Please run `passlane -l` to login (or signup) first.");
    }
    let token = store::get_access_token()?;
    debug!("Token expired? {}", token.is_expired());
    debug!("Token {}", token);
    if token.is_expired() {
        match auth::exchange_refresh_token(token).await {
            Ok(token) => {
                store::store_access_token(&token)?;
                Ok(token)
            }
            Err(err) => {
                warn!("failed to refresh access token: {}", err);
                let token = auth::login()?;
                store::store_access_token(&token)?;
                Ok(token)
            }
        }
    } else {
        Ok(token)
    }
}

async fn push_one_credential(
    master_pwd: &String,
    credentials: &Credentials,
) -> anyhow::Result<i32> {
    let token = get_access_token().await?;
    online_vault::push_one_credential(&token.access_token, &credentials.encrypt(master_pwd), None)
        .await
}

pub fn copy_to_clipboard(value: &String) {
    let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
    ctx.set_contents(String::from(value)).unwrap();
}

#[async_trait]
pub trait Action {
    async fn execute(&self) -> anyhow::Result<()>;
}

pub struct LoginAction {}

impl LoginAction {
    pub fn new() -> LoginAction {
        LoginAction {}
    }
    async fn login(&self) -> anyhow::Result<bool> {
        let token = task::spawn_blocking(move || auth::login()).await??;
        let first_login = !store::has_logged_in();
        store::store_access_token(&token)?;
        Ok(first_login)
    }
}

#[async_trait]
impl Action for LoginAction {
    async fn execute(&self) -> anyhow::Result<()> {
        match self.login().await {
            Ok(is_first_login) => {
                println!("Logged in successfully. Online vaults in use.");
                if is_first_login {
                    println!("You can push all your locally stored credentials to the Online Vault with: passlane push");
                }
            }
            Err(message) => println!("Login failed: {}", message),
        };
        Ok(())
    }
}

pub struct AddAction {
    pub keychain: bool,
    pub generate: bool,
    pub clipboard: bool,
}

impl AddAction {
    pub fn new(matches: &ArgMatches) -> AddAction {
        AddAction {
            keychain: *matches
                .get_one::<bool>("keychain")
                .expect("defaulted to false by clap"),
            generate: *matches
                .get_one::<bool>("generate")
                .expect("defaulted to false by clap"),
            clipboard: *matches
                .get_one::<bool>("clipboard")
                .expect("defaulted to false by clap"),
        }
    }
    fn password_from_clipboard(&self) -> anyhow::Result<String> {
        let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
        let value = ctx
            .get_contents()
            .expect("Unable to retrieve value from clipboard");
        if !password::validate_password(&value) {
            bail!("The text in clipboard is not a valid password");
        }
        Result::Ok(value)
    }
    fn get_password(&self) -> anyhow::Result<String> {
        if self.generate {
            Ok(password::generate())
        } else if self.clipboard {
            self.password_from_clipboard()
        } else {
            Ok(ui::ask_password("Enter password to save: "))
        }
    }
    async fn save(&self, master_pwd: &String, creds: &Credentials) -> anyhow::Result<()> {
        if store::has_logged_in() {
            info!("saving to online vault");
            push_one_credential(master_pwd, &creds).await?;
        } else {
            info!("saving to local file");
            store::save(master_pwd, creds);
        }
        if self.keychain {
            keychain::save(&creds).expect("Unable to store credentials to keychain");
        }
        println!("Saved.");
        Ok(())
    }
}

#[async_trait]
impl Action for AddAction {
    async fn execute(&self) -> anyhow::Result<()> {
        let password = self.get_password().context(format!(
            "Failed to get password {}",
            if self.clipboard { "from clipboard" } else { "" }
        ))?;

        let creds = ui::ask_credentials(&password);
        let master_pwd = ui::ask_master_password(None);
        self.save(&master_pwd, &creds)
            .await
            .context("failed to save")?;
        if !self.clipboard {
            copy_to_clipboard(&password);
            println!("Password - also copied to clipboard: {}", password);
        };
        Ok(())
    }
}

pub struct PushAction {}

async fn push_credentials() -> anyhow::Result<i32> {
    let token = get_access_token().await?;
    let credentials = store::get_all_credentials();
    online_vault::push_credentials(&token.access_token, &credentials, None).await
}

#[async_trait]
impl Action for PushAction {
    async fn execute(&self) -> anyhow::Result<()> {
        match push_credentials().await {
            Ok(num) => println!("Pushed {} credentials online", num),
            Err(message) => println!("Push failed: {}", message),
        };
        Ok(())
    }
}

pub struct ShowAction {
    pub grep: String,
    pub verbose: bool,
}

impl ShowAction {
    pub fn new(matches: &ArgMatches) -> ShowAction {
        ShowAction {
            grep: matches.value_of("REGEXP").expect("required").to_string(),
            verbose: *matches
                .get_one::<bool>("verbose")
                .expect("defaulted to false by clap"),
        }
    }
}

pub async fn find_matches(
    master_pwd: Option<&str>,
    grep_value: &str,
) -> anyhow::Result<Vec<Credentials>> {
    let matches = if store::has_logged_in() {
        info!("searching from online vault");
        let token = get_access_token().await?;
        online_vault::grep(&token.access_token, master_pwd, &grep_value).await?
    } else {
        info!("searching from local file");
        store::grep(master_pwd, grep_value)
    };
    if matches.len() == 0 {
        println!("No matches found");
    }
    Ok(matches)
}

#[async_trait]
impl Action for ShowAction {
    async fn execute(&self) -> anyhow::Result<()> {
        let master_pwd = ui::ask_master_password(None);
        let matches = find_matches(Some(&master_pwd), &self.grep).await?;
        if matches.len() >= 1 {
            println!("Found {} matches:", matches.len());
            ui::show_as_table(&matches, self.verbose);
            if matches.len() == 1 {
                copy_to_clipboard(&matches[0].password);
                println!("Password copied to clipboard!",);
            } else {
                match ui::ask_index(
                    "To copy one of these passwords to clipboard, please enter a row number from the table above, or press q to exit:",
                    &matches,
                ) {
                    Ok(index) => {
                        copy_to_clipboard(&matches[index].password);
                        println!("Password from index {} copied to clipboard!", index);
                    }
                    Err(message) => {
                        println!("{}", message);
                    }
                }
            }
        }
        Ok(())
    }
}
