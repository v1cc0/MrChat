use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use gpui::{
    App, FontWeight, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use tracing::{info, warn};

use crate::player::ui::{
    components::{
        button::{ButtonIntent, button},
        modal::{OnExitHandler, modal},
    },
    models::Models,
    theme::Theme,
};

const SAMPLE_CONFIG: &str = include_str!("../../../config.sample.toml");

#[derive(IntoElement)]
pub struct ConfigDialog {
    on_exit: &'static OnExitHandler,
    config_path: Arc<PathBuf>,
}

impl RenderOnce for ConfigDialog {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let path_string = self.config_path.display().to_string();

        modal()
            .on_exit(self.on_exit)
            .child(
                div()
                    .id("config-setup-dialog")
                    .flex()
                    .flex_col()
                    .w(px(480.0))
                    .p(px(24.0))
                    .gap(px(18.0))
                    .child(
                        div()
                            .text_size(px(20.0))
                            .font_weight(FontWeight::BOLD)
                            .child("需要配置 MrChat"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.text_secondary)
                            .child(
                                div().child(format!(
                                    "未在数据目录找到 config.toml。请在下列路径创建配置文件：{}",
                                    path_string
                                )),
                            )
                            .child(
                                div().mt(px(8.0)).child(
                                    "可以点击“生成默认配置”按钮获取一份模版文件，再根据实际环境填写 API 等参数。",
                                ),
                            )
                            .child(
                                div()
                                    .mt(px(8.0))
                                    .child("配置保存后请重新启动或刷新以启用聊天功能。"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(12.0))
                            .flex_wrap()
                            .child({
                                let path = self.config_path.clone();
                                button()
                                    .intent(ButtonIntent::Primary)
                                    .id("config-generate-default")
                                    .child(SharedString::from("生成默认配置"))
                                    .on_click(move |_, _, cx| {
                                        let show_config = cx.global::<Models>().show_config.clone();
                                        let path = path.clone();
                                        match write_default_config(path.as_ref()) {
                                            Ok(status) => {
                                                match status {
                                                    ConfigWriteStatus::Created => info!(
                                                        "已生成默认配置：{}",
                                                        path.display()
                                                    ),
                                                    ConfigWriteStatus::AlreadyExists => info!(
                                                        "配置文件已存在：{}",
                                                        path.display()
                                                    ),
                                                }
                                            }
                                            Err(err) => {
                                                warn!(
                                                    "写入默认配置失败（{}）：{err:?}",
                                                    path.display()
                                                );
                                            }
                                        }
                                        show_config.write(cx, false);
                                    })
                            })
                            .child(
                                button()
                                    .intent(ButtonIntent::Secondary)
                                    .id("config-dismiss")
                                    .child(SharedString::from("稍后配置"))
                                    .on_click(|_, _, cx| {
                                        let show_config = cx.global::<Models>().show_config.clone();
                                        show_config.write(cx, false);
                                    }),
                            ),
                    ),
            )
    }
}

enum ConfigWriteStatus {
    Created,
    AlreadyExists,
}

fn write_default_config(path: &Path) -> Result<ConfigWriteStatus, std::io::Error> {
    if path.exists() {
        return Ok(ConfigWriteStatus::AlreadyExists);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, SAMPLE_CONFIG)?;
    Ok(ConfigWriteStatus::Created)
}

pub fn config_dialog(config_path: Arc<PathBuf>, on_exit: &'static OnExitHandler) -> ConfigDialog {
    ConfigDialog {
        on_exit,
        config_path,
    }
}
