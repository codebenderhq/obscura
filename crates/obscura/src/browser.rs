use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use deno_core::JsRuntime;
use obscura_browser::{BrowserContext, RuntimeFactory};
use obscura_js::module_loader::ObscuraModuleLoader;
use obscura_net::CookieJar;

use crate::config::BrowserConfig;
use crate::cookie::CookieStore;
use crate::error::Error;
use crate::page::Page;

static NEXT_PAGE_ID: AtomicU64 = AtomicU64::new(1);

pub struct Browser {
    context: Arc<BrowserContext>,
    cookie_jar: Arc<CookieJar>,
    runtime_factory: Option<RuntimeFactory>,
}

impl Browser {
    pub fn new() -> Result<Self, Error> {
        Self::build(BrowserConfig::default())
    }

    pub fn build(config: BrowserConfig) -> Result<Self, Error> {
        let context = if let Some(ref dir) = config.storage_dir {
            BrowserContext::with_storage_full(
                "api".to_string(),
                config.proxy,
                config.stealth,
                config.user_agent,
                Some(dir.clone()),
            )
        } else {
            BrowserContext::with_full_options(
                "api".to_string(),
                config.proxy,
                config.stealth,
                config.user_agent,
            )
        };

        let context = Arc::new(context);
        let cookie_jar = context.cookie_jar.clone();

        Ok(Browser {
            context,
            cookie_jar,
            runtime_factory: None,
        })
    }

    pub fn build_with_runtime_factory(
        config: BrowserConfig,
        runtime_factory: RuntimeFactory,
    ) -> Result<Self, Error> {
        let mut browser = Self::build(config)?;
        browser.runtime_factory = Some(runtime_factory);
        Ok(browser)
    }

    pub fn builder() -> BrowserBuilder {
        BrowserBuilder::default()
    }

    pub async fn new_page(&self) -> Result<Page, Error> {
        let id = NEXT_PAGE_ID.fetch_add(1, Ordering::Relaxed);
        let page = obscura_browser::Page::new_with_runtime_factory(
            format!("page-{}", id),
            self.context.clone(),
            self.runtime_factory.clone(),
        );
        Ok(Page {
            inner: RefCell::new(page),
        })
    }

    /// Access the cookie store for this browser session.
    pub fn cookies(&self) -> CookieStore {
        CookieStore::new(self.cookie_jar.clone())
    }
}

#[derive(Default)]
pub struct BrowserBuilder {
    config: BrowserConfig,
    runtime_factory: Option<RuntimeFactory>,
}

impl BrowserBuilder {
    pub fn runtime_factory(
        mut self,
        factory: impl Fn(Rc<ObscuraModuleLoader>) -> JsRuntime + 'static,
    ) -> Self {
        self.runtime_factory = Some(Arc::new(factory));
        self
    }
    pub fn proxy(mut self, proxy: impl Into<String>) -> Self {
        self.config.proxy = Some(proxy.into());
        self
    }
    pub fn stealth(mut self, stealth: bool) -> Self {
        self.config.stealth = stealth;
        self
    }
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.config.user_agent = Some(ua.into());
        self
    }
    pub fn storage_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.config.storage_dir = Some(dir.into());
        self
    }
    pub fn build(self) -> Result<Browser, Error> {
        match self.runtime_factory {
            Some(factory) => Browser::build_with_runtime_factory(self.config, factory),
            None => Browser::build(self.config),
        }
    }
}
