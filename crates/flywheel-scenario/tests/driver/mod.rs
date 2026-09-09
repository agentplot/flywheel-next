//! The 390px driver: the page served from the process that just ticked, opened
//! in a headless Chromium at a chosen viewport, and a control tapped (314,
//! D15).
//!
//! The browser is a test dependency of this crate and of no shipped crate: the
//! binary a host runs links no browser. The page is served from the store the
//! run left behind, on a loopback port of its own, so what the driver reads is
//! what the tick wrote and nothing is fetched from anywhere else (310).

#![allow(dead_code)]

use anyhow::{Context, Result};
use flywheel_scenario::store::Store;
use flywheel_engine::Definitions;
use headless_chrome::protocol::cdp::Emulation;
use headless_chrome::{Browser, LaunchOptions, Tab};
use std::sync::Arc;

/// The phone, and the desktop the same assertions run at (314).
pub const PHONE: (u32, u32) = (390, 844);
pub const DESKTOP: (u32, u32) = (1440, 900);

/// The page, served from the process that just ticked.
///
/// The store is moved in, because the served page reads it on every request and
/// a second reader would be a second source of truth (15, 310). What the
/// assertions need back from it is read through the page.
pub struct Served {
    /// Where the page is, on loopback.
    pub url: String,
    /// Kept so the runtime lives as long as the page is served; dropping it
    /// ends the server.
    _runtime: tokio::runtime::Runtime,
}

impl Served {
    /// Serve the page from a store the run left behind. The address every link
    /// is written at stays the host's own, which is what a link on a phone
    /// opens (205a, D10a); the loopback port is what the driver reaches.
    pub fn page(store: Store, defs: Definitions, operator: &str) -> Result<Served> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .context("a runtime for the served page")?;
        let mut served = flywheel_surface::http::Served::over(
            store,
            Box::new(flywheel_scenario::bindings::FilesWorld::new()),
            defs,
            &[operator.to_string()],
            "http://flywheel.local/willdan",
        );
        let listener = runtime
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .context("binding a loopback port for the page")?;
        let port = listener.local_addr()?.port();
        // The driver is the operator sitting at the machine, which is the
        // localhost port 245 permits beside the private-network address; the
        // page is served unsigned-in there and nowhere else (155, 253a).
        served.localhost_port = port;
        runtime.spawn(async move {
            let _ = flywheel_surface::http::serve_on(served, listener).await;
        });
        Ok(Served {
            url: format!("http://127.0.0.1:{port}/"),
            _runtime: runtime,
        })
    }
}

/// The browser the pass opens the rail in. One per run: the viewport is set per
/// visit, so a phone pass and a desktop pass are two visits and not two
/// browsers.
pub struct Driver {
    browser: Browser,
    tab: Arc<Tab>,
}

/// Whether a browser is there to drive. The driver needs one installed; where
/// there is none the pass says so rather than failing as though the page were
/// wrong.
pub fn available() -> bool {
    headless_chrome::browser::default_executable().is_ok()
}

impl Driver {
    pub fn open() -> Result<Driver> {
        let options = LaunchOptions::default_builder()
            .headless(true)
            .window_size(Some(PHONE))
            .build()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let browser = Browser::new(options).context("starting the headless browser")?;
        let tab = browser.new_tab().context("opening a tab")?;
        tab.set_default_timeout(std::time::Duration::from_secs(10));
        Ok(Driver { browser, tab })
    }

    /// Open a page at a viewport. The metrics are overridden rather than the
    /// window resized, so the same browser serves both viewports and the media
    /// query the bundle carries decides the layout (307).
    pub fn visit(&self, url: &str, (width, height): (u32, u32)) -> Result<&Arc<Tab>> {
        self.tab
            .call_method(Emulation::SetDeviceMetricsOverride {
                width,
                height,
                device_scale_factor: 1.0,
                // The layout viewport is the width asked for, not a mobile
                // browser's 980px fallback: what is under test is the bundle's
                // own media query at 390px (307).
                mobile: false,
                scale: None,
                screen_width: Some(width),
                screen_height: Some(height),
                position_x: None,
                position_y: None,
                dont_set_visible_size: Some(false),
                screen_orientation: None,
                viewport: None,
                display_feature: None,
                device_posture: None,
            })
            .context("setting the viewport")?;
        self.tab.navigate_to(url)?;
        self.tab.wait_until_navigated()?;
        Ok(&self.tab)
    }

    pub fn tab(&self) -> &Arc<Tab> {
        &self.tab
    }
}

/// The width the page laid itself out at, as the document reports it. What a
/// media query read, so an assertion about the phone layout is about the layout
/// and not about what the driver asked for.
pub fn viewport_width(tab: &Tab) -> Result<i64> {
    let value = tab.evaluate("document.documentElement.clientWidth", false)?;
    Ok(value
        .value
        .and_then(|v| v.as_i64())
        .context("the document reports its width")?)
}

/// What the page reports about one control: whether it is laid out, how big it
/// is, and whether a tap at its centre reaches it or something else.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Measured {
    pub width: f64,
    pub height: f64,
    pub left: f64,
    pub top: f64,
    pub shown: bool,
    /// Whether a tap at the control's centre lands on it, so nothing covers it.
    pub reached: bool,
    /// Whether the control only appears once something is hovered, which a
    /// finger cannot do (311).
    pub behind_hover: bool,
}

impl Measured {
    /// A finger can reach it: it is on the page, nothing covers it, it is at
    /// least the 44px a tap target needs, and it is not behind a hover (311).
    pub fn tappable(&self) -> bool {
        self.shown
            && self.reached
            && !self.behind_hover
            && self.width >= 44.0
            && self.height >= 44.0
            && self.left >= 0.0
            && self.top >= 0.0
    }
}

/// Measure one control. The page is asked, so what is asserted is the layout
/// the browser made and not the markup the renderer wrote.
pub fn measure(tab: &Tab, selector: &str) -> Result<Option<Measured>> {
    let script = format!(
        "JSON.stringify((() => {{ const el = document.querySelector({selector:?}); \
          if (!el) return null; \
          el.scrollIntoView({{block: 'center'}}); \
          const r = el.getBoundingClientRect(); \
          const style = getComputedStyle(el); \
          const at = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2); \
          const rules = [...document.styleSheets].flatMap(s => {{ try {{ return [...s.cssRules]; }} \
            catch (e) {{ return []; }} }}); \
          const hover = rules.some(rule => rule.selectorText \
            && rule.selectorText.includes(':hover') \
            && [...document.querySelectorAll(rule.selectorText.replace(/:hover/g, ''))] \
                 .some(other => other === el || other.contains(el))); \
          return {{ width: r.width, height: r.height, left: r.left, top: r.top, \
                    shown: style.display !== 'none' && style.visibility !== 'hidden' \
                           && style.opacity !== '0', \
                    reached: at !== null && (at === el || el.contains(at) || at.contains(el)), \
                    behind_hover: hover }}; }})())"
    );
    let value = tab.evaluate(&script, false)?;
    let text = value
        .value
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .context("the page answers what it measured")?;
    if text == "null" {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(&text)?))
}

/// Whether a control is where a finger can reach it (311).
pub fn tappable(tab: &Tab, selector: &str) -> Result<bool> {
    Ok(measure(tab, selector)?.is_some_and(|m| m.tappable()))
}

/// Wait until the tab is somewhere else. A tap on an answer posts a form, and
/// the page it lands on is what says whether the tool recorded the call; the
/// navigation is the browser's own and is waited for rather than assumed.
pub fn wait_for_url(tab: &Tab, holding: &str) -> Result<String> {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < until {
        let now = tab.get_url();
        if now != holding {
            return Ok(now);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    anyhow::bail!("the tab stayed at {holding} after the tap")
}
