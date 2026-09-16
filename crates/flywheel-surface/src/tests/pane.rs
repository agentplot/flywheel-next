//! A session chip's pane link says where the pane is (S53, S234, 174, 196).
//!
//! The page cannot drive a terminal, so the link carries what the popover
//! shows: the herdr session the pane is in, the host it runs on, the pane by
//! its session id, and the two lines to copy. A pane that is gone says so and
//! offers nothing to copy.

use crate::page::pane_link;
use crate::page::Session;

fn a_session() -> Session {
    Session {
        id: "curation/agentplot/main/1".into(),
        item: "curation/agentplot".into(),
        host: "studio".into(),
        runner: "herdr".into(),
        agent: Some("curation-agentplot-main-1".into()),
        pane: Some("w2:p1".into()),
        herdr_session: Some("flywheel-agentplot-machinery".into()),
        started: Some("2026-09-15T08:00:00Z".into()),
        ..Default::default()
    }
}

/// The link names the session, the host and the pane, and carries both lines
/// with their copy controls (S234).
#[test]
fn a_pane_link_names_its_session_host_and_pane_with_lines_to_copy() {
    let link = pane_link(&a_session(), "studio");

    assert!(link.contains("data-sess-session=\"flywheel-agentplot-machinery\""), "{link}");
    assert!(link.contains("data-sess-host=\"studio\""), "{link}");
    assert!(link.contains("data-pane=\"w2:p1\""), "{link}");
    assert!(link.contains("data-sess-id=\"curation/agentplot/main/1\""), "{link}");
    // The two lines a person copies to reach it.
    assert!(
        link.contains("data-sess-attach=\"herdr session attach flywheel-agentplot-machinery\""),
        "the attach line is not there: {link}"
    );
    assert!(
        link.contains("data-sess-focus=\"herdr agent focus curation/agentplot/main/1\""),
        "the focus line is not there: {link}"
    );
    // On this computer, so no remote prefix and nothing said about another.
    assert!(!link.contains("data-sess-remote"), "{link}");
    assert!(!link.contains("--remote"), "{link}");
}

/// A session on another computer is reached through herdr's remote attach,
/// naming the host's machine (S234, 232).
#[test]
fn a_remote_hosts_attach_line_names_its_machine() {
    let link = pane_link(&a_session(), "laptop");

    assert!(link.contains("data-sess-remote=\"1\""), "{link}");
    assert!(
        link.contains("data-sess-attach=\"herdr --remote studio --session flywheel-agentplot-machinery\""),
        "the remote attach line does not name the machine: {link}"
    );
    // The focus line is the same wherever the pane is.
    assert!(link.contains("data-sess-focus=\"herdr agent focus curation/agentplot/main/1\""), "{link}");
}

/// A pane that is gone says so and offers nothing to copy (68).
#[test]
fn a_gone_pane_offers_nothing_to_copy() {
    // Exited: after 74 its pane closed on the pass that recorded the exit.
    let mut exited = a_session();
    exited.exit = Some("done".into());
    exited.exit_at = Some("2026-09-15T08:12:00Z".into());
    let link = pane_link(&exited, "studio");
    assert!(link.contains("no pane"), "{link}");
    assert!(link.contains("08:12"), "the line says since when: {link}");
    assert!(!link.contains("data-sess-attach"), "a gone pane offers nothing to copy: {link}");
    assert!(!link.contains("data-sess-focus"), "{link}");

    // Lost: the pane went without an exit, and there is nothing to copy either.
    let mut lost = a_session();
    lost.pane = None;
    let link = pane_link(&lost, "studio");
    assert!(link.contains("no pane"), "{link}");
    assert!(link.contains("lost"), "{link}");
    assert!(!link.contains("data-sess-attach"), "{link}");
}

/// The bundle's own file, as the host serves it.
fn bundled(address: String) -> &'static str {
    let file = address.rsplit('/').next().expect("a name").to_string();
    crate::page::bundled(&file).expect("the served bundle").1
}

/// The popover closes as a picker does: on Esc, a click anywhere outside it,
/// another picker, a dock page or the palette — and on a second press of its
/// own chip. The palette is the one that could be raised by a key rather than
/// a click, so it closes the popover itself (S234, S233, S56, S57).
#[test]
fn the_palette_closes_an_open_pane_popover() {
    let js = bundled(crate::page::script_address());

    let opens = js
        .split("function openBox(")
        .nth(1)
        .and_then(|rest| rest.split("function closeBox(").next())
        .expect("the palette's own opening is in the bundle");
    assert!(
        opens.contains("closePanePop()"),
        "raising the palette leaves a pane popover open (S234): {opens}"
    );

    // And the rest of the ways it goes, which a click or a key reaches.
    assert!(
        js.contains("addEventListener('hashchange', closePanePop)"),
        "a dock page opening leaves the popover open (S234)"
    );
    assert!(
        js.contains("if(panepop && panepop.innerHTML && !(t.closest && t.closest('#panepop'))) closePanePop();"),
        "a click outside — another picker among them — leaves the popover open (S234)"
    );
    assert!(
        js.contains("if(e.key==='Escape' && panepop && panepop.innerHTML)"),
        "Esc does not close the popover first (S56)"
    );
    assert!(
        js.contains("if(panepop.getAttribute('data-for')===d.sessId && panepop.innerHTML){ closePanePop(); return; }"),
        "a second press on the same chip does not close it (S234)"
    );
}

/// On a phone the popover is a bottom sheet, as the palette and a picker are:
/// a chip is too small to hang a 380px panel off, and a sheet above the tabs
/// is what a thumb reaches (S234, S233, S30, 306).
#[test]
fn a_pane_popover_is_a_bottom_sheet_under_760px() {
    let css = bundled(crate::page::style_address());
    let (desktop, phone) = css
        .split_once("@media (max-width: 760px)")
        .expect("the bundle lays out under 760px");

    assert!(
        desktop.contains(".panepop{position:fixed;"),
        "the popover is not placed at all on the desktop: {desktop}"
    );
    assert!(
        phone.contains(".panepop{left:8px!important;right:8px;top:auto!important;bottom:58px;width:auto;border-radius:12px}"),
        "the popover is still anchored under its chip at phone width (S234): {phone}"
    );
}
