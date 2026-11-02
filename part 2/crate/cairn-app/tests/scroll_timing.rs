//! How long a frame takes to build while scrolling the Sales Orders list,
//! all 13,531 placeholder orders. Build time only: the drawing happens on
//! the GPU afterwards. Reads the dev database, so it's ignored by default:
//!
//!   cargo test --release -p cairn-app --test scroll_timing -- --ignored --nocapture

use cairn_app::app::CairnApp;
use cairn_app::context::{AppContext, Target};
use cairn_app::data;
use egui::{pos2, vec2, Event, Modifiers, MouseWheelUnit};
use egui_kittest::kittest::{by, Queryable};
use egui_kittest::Harness;
use std::cell::RefCell;
use std::time::{Duration, Instant};

#[test]
#[ignore = "reads the dev database"]
fn scrolling_13531_orders() {
    let url = std::env::var("CAIRN_DATABASE_URL")
        .unwrap_or_else(|_| data::DEV_DATABASE_URL.into());
    let mut app =
        CairnApp::new(AppContext::new(data::load(&url).unwrap()));
    app.open(Target::OrderList);
    let rows = app.ctx.data.orders.len();
    let times = RefCell::new(Vec::<Duration>::new());
    let mut h = Harness::builder()
        .with_size([1180.0, 760.0])
        .build_ui_state(
            |ui, app: &mut CairnApp| {
                let start = Instant::now();
                app.show(ui);
                times.borrow_mut().push(start.elapsed());
            },
            app,
        );
    h.run();
    times.borrow_mut().clear();
    h.hover_at(pos2(400.0, 400.0));
    // Fling to the bottom, about 30 rows a frame, until the oldest order
    // (last in the list) is on screen.
    let oldest =
        h.state().ctx.data.orders.last().unwrap().number.clone();
    let on_screen = |h: &Harness<'_, CairnApp>| {
        h.root()
            .query_all(by().predicate(|n| {
                n.value().is_some_and(|v| v == oldest)
            }))
            .next()
            .is_some()
    };
    let mut frames = 0;
    while !on_screen(&h) {
        assert!(frames < 2_000, "didn't reach {oldest}");
        h.event(Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(0.0, -600.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
            source: egui::MouseWheelSource::Wheel,
        });
        h.step();
        frames += 1;
    }
    let mut t = times.borrow().clone();
    t.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!(
        "{rows} orders, {} frames: median {:.3} ms, 99th percentile {:.3} ms, max {:.3} ms",
        t.len(),
        ms(t[t.len() / 2]),
        ms(t[t.len() * 99 / 100]),
        ms(t[t.len() - 1]),
    );
}

/// How long the largest datasheet view, 68,257 activities, takes to
/// build when it opens: every field turned into text once.
#[test]
#[ignore = "reads the dev database"]
fn opening_the_activities_datasheet() {
    use cairn_app::screens::sheet::{Sheet, SheetOf};
    let url = std::env::var("CAIRN_DATABASE_URL")
        .unwrap_or_else(|_| data::DEV_DATABASE_URL.into());
    let ctx = AppContext::new(data::load(&url).unwrap());
    let start = Instant::now();
    let _sheet = Sheet::build(SheetOf::Activities, &ctx);
    println!(
        "{} activities: datasheet built in {:.0} ms",
        ctx.data.activities.len(),
        start.elapsed().as_secs_f64() * 1000.0
    );
}
