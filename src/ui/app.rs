use core::panic;
use std::{
    fs,
    sync::{Arc, RwLock},
};

use directories::ProjectDirs;
use gpui::*;
use prelude::FluentBuilder;
use sqlx::SqlitePool;
use tracing::{debug, warn};

use crate::{
    library::{
        db::create_pool,
        scan::{ScanInterface, ScanThread},
    },
    modules::chat::{self, services::ChatServices, ui::layout::ChatOverview},
    playback::{interface::GPUIPlaybackInterface, queue::QueueItemData, thread::PlaybackThread},
    services::controllers::make_cl,
    settings::{
        SettingsGlobal, setup_settings,
        storage::{Storage, StorageData},
    },
    shared::db::{TursoConfig, TursoPool},
    ui::{assets::HummingbirdAssetSource, constants::APP_SHADOW_SIZE},
};

use super::{
    about::about_dialog,
    arguments::parse_args_and_prepare,
    components::{input, modal},
    constants::APP_ROUNDING,
    controls::Controls,
    data::create_album_cache,
    global_actions::register_actions,
    header::Header,
    library::Library,
    models::{self, Models, PlaybackInfo, build_models},
    queue::Queue,
    search::SearchView,
    theme::{Theme, setup_theme},
    util::drop_image_from_app,
};

struct WindowShadow {
    pub controls: Entity<Controls>,
    pub queue: Entity<Queue>,
    pub library: Entity<Library>,
    pub header: Entity<Header>,
    pub search: Entity<SearchView>,
    pub chat_overview: Entity<ChatOverview>,
    pub show_queue: Entity<bool>,
    pub show_about: Entity<bool>,
}

impl Render for WindowShadow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();

        let decorations = window.window_decorations();
        let rounding = APP_ROUNDING;
        let shadow_size = APP_SHADOW_SIZE;
        let border_size = px(1.0);
        window.set_client_inset(shadow_size);

        let queue = self.queue.clone();
        let show_queue_flag = *self.show_queue.read(cx);
        let show_about = *self.show_about.clone().read(cx);
        let chat_overview = self.chat_overview.clone();

        let mut element = div()
            .id("window-backdrop")
            .key_context("app")
            .bg(transparent_black())
            .flex()
            .map(|div| match decorations {
                Decorations::Server => div,
                Decorations::Client { tiling, .. } => div
                    .bg(gpui::transparent_black())
                    .child(
                        canvas(
                            |_bounds, window, _| {
                                window.insert_hitbox(
                                    Bounds::new(
                                        point(px(0.0), px(0.0)),
                                        window.window_bounds().get_bounds().size,
                                    ),
                                    HitboxBehavior::Normal,
                                )
                            },
                            move |_bounds, hitbox, window, _| {
                                let mouse = window.mouse_position();
                                let size = window.window_bounds().get_bounds().size;
                                let Some(edge) = resize_edge(mouse, px(30.0), size, tiling) else {
                                    return;
                                };
                                window.set_cursor_style(
                                    match edge {
                                        ResizeEdge::Top | ResizeEdge::Bottom => {
                                            CursorStyle::ResizeUpDown
                                        }
                                        ResizeEdge::Left | ResizeEdge::Right => {
                                            CursorStyle::ResizeLeftRight
                                        }
                                        ResizeEdge::TopLeft | ResizeEdge::BottomRight => {
                                            CursorStyle::ResizeUpLeftDownRight
                                        }
                                        ResizeEdge::TopRight | ResizeEdge::BottomLeft => {
                                            CursorStyle::ResizeUpRightDownLeft
                                        }
                                    },
                                    &hitbox,
                                );
                            },
                        )
                        .size_full()
                        .absolute(),
                    )
                    .when(!(tiling.top || tiling.right), |div| {
                        div.rounded_tr(rounding)
                    })
                    .when(!(tiling.top || tiling.left), |div| div.rounded_tl(rounding))
                    .when(!(tiling.bottom || tiling.right), |div| {
                        div.rounded_br(rounding)
                    })
                    .when(!(tiling.bottom || tiling.left), |div| {
                        div.rounded_bl(rounding)
                    })
                    .when(!tiling.top, |div| div.pt(shadow_size))
                    .when(!tiling.bottom, |div| div.pb(shadow_size))
                    .when(!tiling.left, |div| div.pl(shadow_size))
                    .when(!tiling.right, |div| div.pr(shadow_size))
                    .on_mouse_down(MouseButton::Left, move |e, window, _| {
                        let size = window.window_bounds().get_bounds().size;
                        let pos = e.position;

                        if let Some(edge) = resize_edge(pos, shadow_size, size, tiling) {
                            window.start_window_resize(edge)
                        };
                    }),
            })
            .size_full()
            .child(
                div()
                    .font_family("Inter")
                    .text_color(theme.text)
                    .cursor(CursorStyle::Arrow)
                    .map(|div| match decorations {
                        Decorations::Server => div,
                        Decorations::Client { tiling } => div
                            .when(cfg!(not(target_os = "macos")), |div| {
                                div.border_color(rgba(0x64748b33))
                            })
                            .when(!(tiling.top || tiling.right), |div| {
                                div.rounded_tr(rounding)
                            })
                            .when(!(tiling.top || tiling.left), |div| div.rounded_tl(rounding))
                            .when(!(tiling.bottom || tiling.right), |div| {
                                div.rounded_br(rounding)
                            })
                            .when(!(tiling.bottom || tiling.left), |div| {
                                div.rounded_bl(rounding)
                            })
                            .when(!tiling.top, |div| div.border_t(border_size))
                            .when(!tiling.bottom, |div| div.border_b(border_size))
                            .when(!tiling.left, |div| div.border_l(border_size))
                            .when(!tiling.right, |div| div.border_r(border_size))
                            .when(!tiling.is_tiled(), |div| {
                                div.shadow(vec![gpui::BoxShadow {
                                    color: Hsla {
                                        h: 0.,
                                        s: 0.,
                                        l: 0.,
                                        a: 0.4,
                                    },
                                    blur_radius: shadow_size / 2.,
                                    spread_radius: px(0.),
                                    offset: point(px(0.0), px(0.0)),
                                }])
                            }),
                    })
                    .on_mouse_move(|_e, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_drop(|ev: &ExternalPaths, _, cx| {
                        let items = ev
                            .paths()
                            .iter()
                            .map(|path| QueueItemData::new(cx, path.clone(), None, None))
                            .collect();

                        let playback_interface = cx.global::<GPUIPlaybackInterface>();
                        playback_interface.queue_list(items);
                    })
                    .overflow_hidden()
                    .bg(theme.background_primary)
                    .size_full()
                    .flex()
                    .flex_col()
                    .max_w_full()
                    .max_h_full()
                    .child(self.header.clone())
                    .child({
                        let mut library_column = div()
                            .flex()
                            .flex_col()
                            .flex_grow()
                            .max_w_full()
                            .max_h_full()
                            .overflow_hidden()
                            .child(self.library.clone());

                        if show_queue_flag {
                            library_column = library_column.child(queue);
                        }

                        let chat_column = div()
                            .flex()
                            .flex_col()
                            .flex_basis(px(340.0))
                            .flex_shrink_0()
                            .max_h_full()
                            .bg(theme.background_secondary)
                            .rounded(px(12.0))
                            .p(px(16.0))
                            .child(chat_overview);

                        div()
                            .flex()
                            .gap(px(20.0))
                            .w_full()
                            .h_full()
                            .max_w_full()
                            .max_h_full()
                            .child(library_column)
                            .child(chat_column)
                    })
                    .child(self.controls.clone())
                    .child(self.search.clone())
                    .when(show_about, |this| {
                        this.child(about_dialog(&|_, cx| {
                            let show_about = cx.global::<Models>().show_about.clone();
                            show_about.write(cx, false);
                        }))
                    }),
            );

        let text_styles = element.text_style();
        *text_styles = Some(TextStyleRefinement::default());

        let ff = &mut text_styles.as_mut().unwrap().font_features;
        *ff = Some(FontFeatures(Arc::new(vec![("tnum".to_string(), 1)])));

        element
    }
}

fn resize_edge(
    pos: Point<Pixels>,
    shadow_size: Pixels,
    size: Size<Pixels>,
    tiling: Tiling,
) -> Option<ResizeEdge> {
    let edge = if pos.y < shadow_size * 2 && pos.x < shadow_size * 2 && !tiling.top && !tiling.left
    {
        ResizeEdge::TopLeft
    } else if pos.y < shadow_size * 2
        && pos.x > size.width - shadow_size * 2
        && !tiling.top
        && !tiling.right
    {
        ResizeEdge::TopRight
    } else if pos.y < shadow_size && !tiling.top {
        ResizeEdge::Top
    } else if pos.y > size.height - shadow_size * 2
        && pos.x < shadow_size * 2
        && !tiling.bottom
        && !tiling.left
    {
        ResizeEdge::BottomLeft
    } else if pos.y > size.height - shadow_size * 2
        && pos.x > size.width - shadow_size * 2
        && !tiling.bottom
        && !tiling.right
    {
        ResizeEdge::BottomRight
    } else if pos.y > size.height - shadow_size && !tiling.bottom {
        ResizeEdge::Bottom
    } else if pos.x < shadow_size && !tiling.left {
        ResizeEdge::Left
    } else if pos.x > size.width - shadow_size && !tiling.right {
        ResizeEdge::Right
    } else {
        return None;
    };
    Some(edge)
}

pub fn find_fonts(cx: &mut App) -> gpui::Result<()> {
    let paths = cx.asset_source().list("!bundled:fonts")?;
    let mut fonts = vec![];
    for path in paths {
        if path.ends_with(".ttf") || path.ends_with(".otf") {
            if let Some(v) = cx.asset_source().load(&path)? {
                fonts.push(v);
            }
        }
    }

    let results = cx.text_system().add_fonts(fonts);
    debug!("loaded fonts: {:?}", cx.text_system().all_font_names());
    results
}

pub struct Pool(pub SqlitePool);

impl Global for Pool {}

pub fn get_dirs() -> ProjectDirs {
    let secondary_dirs = directories::ProjectDirs::from("me", "william341", "muzak")
        .expect("couldn't generate project dirs (secondary)");

    if secondary_dirs.data_dir().exists() {
        return secondary_dirs;
    }

    directories::ProjectDirs::from("org", "mailliw", "hummingbird")
        .expect("couldn't generate project dirs")
}

pub struct DropImageDummyModel;

impl EventEmitter<Vec<Arc<RenderImage>>> for DropImageDummyModel {}

pub async fn run() {
    let dirs = get_dirs();
    let directory = dirs.data_dir().to_path_buf();
    if !directory.exists() {
        fs::create_dir_all(&directory)
            .unwrap_or_else(|e| panic!("couldn't create data directory, {:?}, {:?}", directory, e));
    }
    let file = directory.join("library.db");

    let pool_result = create_pool(file).await;
    let Ok(pool) = pool_result else {
        panic!(
            "fatal: unable to create database pool: {:?}",
            pool_result.unwrap_err()
        );
    };

    let turso_pool = match TursoConfig::from_env() {
        Ok(config) => match TursoPool::connect(config).await {
            Ok(pool) => Some(Arc::new(pool)),
            Err(err) => {
                warn!("chat module disabled: failed to connect to Turso ({err:?})");
                None
            }
        },
        Err(err) => {
            warn!("chat module disabled: Turso configuration missing ({err:?})");
            None
        }
    };

    let chat_pool = turso_pool.clone();

    Application::new()
        .with_assets(HummingbirdAssetSource::new(pool.clone()))
        .run(move |cx: &mut App| {
            chat::ensure_state_registered(cx);

            if let Some(pool) = chat_pool.clone() {
                let services = ChatServices::new(pool);
                cx.set_global(services.clone());
                chat::bootstrap_state(cx, services);
            }

            let bounds = Bounds::centered(None, size(px(1024.0), px(700.0)), cx);
            find_fonts(cx).expect("unable to load fonts");
            register_actions(cx);

            let queue: Arc<RwLock<Vec<QueueItemData>>> = Arc::new(RwLock::new(Vec::new()));
            let storage = Storage::new(directory.clone().join("app_data.json"));
            let storage_data = storage.load_or_default();

            setup_theme(cx, directory.join("theme.json"));
            setup_settings(cx, directory.join("settings.json"));

            build_models(
                cx,
                models::Queue {
                    data: queue.clone(),
                    position: 0,
                },
                &storage_data,
            );

            input::bind_actions(cx);
            modal::bind_actions(cx);

            create_album_cache(cx);

            let settings = cx.global::<SettingsGlobal>().model.read(cx);
            let playback_settings = settings.playback.clone();
            let mut scan_interface: ScanInterface =
                ScanThread::start(pool.clone(), settings.scanning.clone());
            scan_interface.scan();
            scan_interface.start_broadcast(cx);

            cx.set_global(scan_interface);
            cx.set_global(Pool(pool));

            let drop_model = cx.new(|_| DropImageDummyModel);

            cx.subscribe(&drop_model, |_, vec, cx| {
                for image in vec.clone() {
                    drop_image_from_app(cx, image);
                }
            })
            .detach();

            let mut playback_interface: GPUIPlaybackInterface =
                PlaybackThread::start(queue, playback_settings);
            playback_interface.start_broadcast(cx);

            if !parse_args_and_prepare(cx, &playback_interface) {
                if let Some(track) = storage_data.current_track {
                    // open current track,
                    playback_interface.open(track.get_path().clone());
                    // but stop it immediately
                    playback_interface.pause();
                }
            }
            cx.set_global(playback_interface);

            cx.activate(true);

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_background: WindowBackgroundAppearance::Opaque,
                    window_decorations: Some(WindowDecorations::Client),
                    window_min_size: Some(size(px(800.0), px(600.0))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from("Hummingbird")),
                        appears_transparent: true,
                        traffic_light_position: Some(Point {
                            x: px(12.0),
                            y: px(11.0),
                        }),
                    }),
                    app_id: Some("org.mailliw.hummingbird".to_string()),
                    kind: WindowKind::Normal,
                    ..Default::default()
                },
                |window, cx| {
                    window.set_window_title("Hummingbird");

                    make_cl(cx, window);

                    cx.new(|cx| {
                        cx.observe_window_appearance(window, |_, _, cx| {
                            cx.refresh_windows();
                        })
                        .detach();

                        // Update `StorageData` and save it to file system while quitting the app
                        cx.on_app_quit({
                            let current_track = cx.global::<PlaybackInfo>().current_track.clone();
                            move |_, cx| {
                                let current_track = current_track.read(cx).clone();
                                let storage = storage.clone();
                                cx.background_executor().spawn(async move {
                                    storage.save(&StorageData { current_track });
                                })
                            }
                        })
                        .detach();

                        let show_queue = cx.new(|_| true);
                        let show_about = cx.global::<Models>().show_about.clone();

                        cx.observe(&show_about, |_, _, cx| {
                            cx.notify();
                        })
                        .detach();

                        WindowShadow {
                            controls: Controls::new(cx, show_queue.clone()),
                            queue: Queue::new(cx, show_queue.clone()),
                            library: Library::new(cx),
                            header: Header::new(cx),
                            search: SearchView::new(cx),
                            chat_overview: cx.new(|_| ChatOverview::new()),
                            show_queue,
                            show_about,
                        }
                    })
                },
            )
            .unwrap();
        });
}
