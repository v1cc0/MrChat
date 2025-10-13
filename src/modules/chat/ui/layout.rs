use std::time::{Duration, SystemTime};

use gpui::{
    App, AppContext, Context, CursorStyle, Entity, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    WeakEntity, Window, div, px, rgba,
};
use tracing::warn;

use crate::{
    modules::chat::{
        self,
        models::{ChatState, ConversationId, MessageRole},
        services::ChatServices,
    },
    ui::components::{
        button::{ButtonIntent, ButtonSize, ButtonStyle, button},
        input::{EnrichedInputAction, TextInput},
    },
};

const DEFAULT_CHAT_TITLE: &str = "新会话";
const DEFAULT_CHAT_MODEL: &str = "default";

pub struct ChatOverview {
    input: Entity<TextInput>,
    buffer: Entity<String>,
    focus: FocusHandle,
}

impl ChatOverview {
    pub fn create(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let focus = cx.focus_handle();
            let weak: WeakEntity<Self> = cx.weak_entity();
            let buffer = cx.new(|_| String::new());

            let buffer_clone = buffer.clone();
            let handler = {
                let weak = weak.clone();
                move |action: EnrichedInputAction, _: &mut Window, cx: &mut App| {
                    if matches!(action, EnrichedInputAction::Accept) {
                        if let Some(entity) = weak.upgrade() {
                            let _ = entity.update(cx, |this, cx| this.submit_message(cx));
                        }
                    }
                }
            };

            let input = TextInput::new(
                cx,
                focus.clone(),
                None,
                Some(SharedString::from("发送消息…")),
                Some(Box::new(handler)),
            );

            cx.subscribe(&input, move |_, _, text: &String, cx| {
                buffer_clone.update(cx, |buf, _| {
                    *buf = text.clone();
                });
            })
            .detach();

            ChatOverview {
                input,
                buffer,
                focus,
            }
        })
    }

    fn submit_message(&self, cx: &mut Context<Self>) {
        let Some(services) = cx.try_global::<ChatServices>().cloned() else {
            return;
        };

        let message_text = cx.read_entity(&self.buffer, |value, _| value.clone());
        let trimmed = message_text.trim();
        if trimmed.is_empty() {
            return;
        }
        let text = trimmed.to_string();

        cx.update_entity(&self.buffer, |buf, _| buf.clear());
        cx.update_entity(&self.input, |input, cx| {
            input.reset();
            cx.notify();
        });

        cx.spawn({
            let services = services.clone();
            async move |_weak: WeakEntity<Self>, app| {
                let mut conversation_id = app
                    .update(|app| {
                        let state = app.global::<ChatState>();
                        state.current_conversation.read(app).clone()
                    })
                    .unwrap_or(None);

                if conversation_id.is_none() {
                    match services
                        .create_conversation(DEFAULT_CHAT_TITLE, DEFAULT_CHAT_MODEL)
                        .await
                    {
                        Ok(summary) => {
                            let new_id = summary.id.clone();
                            let summary_clone = summary.clone();

                            let _ = app.update(|app| {
                                let state = app.global::<ChatState>();
                                let conversations = state.conversations.clone();
                                let current = state.current_conversation.clone();
                                let messages = state.messages.clone();

                                conversations.update(app, |list, cx| {
                                    list.insert(0, summary_clone);
                                    cx.notify();
                                });
                                current.update(app, |slot, cx| {
                                    *slot = Some(new_id.clone());
                                    cx.notify();
                                });
                                messages.update(app, |msgs, cx| {
                                    msgs.clear();
                                    cx.notify();
                                });
                            });

                            conversation_id = Some(new_id);
                        }
                        Err(err) => {
                            warn!("failed to create conversation: {err:?}");
                            return;
                        }
                    }
                }

                let Some(conv_id) = conversation_id.clone() else {
                    return;
                };

                match services
                    .append_message(conv_id.clone(), MessageRole::User, text.clone(), None)
                    .await
                {
                    Ok(message) => {
                        let _ = app.update(|app| {
                            let state = app.global::<ChatState>();
                            let conversations = state.conversations.clone();
                            let messages = state.messages.clone();

                            messages.update(app, |msgs, cx| {
                                msgs.push(message.clone());
                                cx.notify();
                            });
                            conversations.update(app, |list, cx| {
                                if let Some(pos) = list.iter().position(|c| c.id == conv_id) {
                                    list[pos].updated_at = SystemTime::now();
                                    let updated = list.remove(pos);
                                    list.insert(0, updated);
                                    cx.notify();
                                }
                            });
                        });
                        // Placeholder assistant echo until real LLM is integrated
                        let services_clone = services.clone();
                        app.background_executor()
                            .timer(Duration::from_millis(300))
                            .await;

                        match services_clone
                            .append_message(
                                conv_id.clone(),
                                MessageRole::Assistant,
                                format!("(LLM TODO) 回显: {}", text),
                                None,
                            )
                            .await
                        {
                            Ok(assistant_msg) => {
                                let _ = app.update(|app| {
                                    let state = app.global::<ChatState>();
                                    let messages = state.messages.clone();
                                    messages.update(app, |msgs, cx| {
                                        msgs.push(assistant_msg.clone());
                                        cx.notify();
                                    });
                                });
                            }
                            Err(err) => warn!("failed to append assistant message: {err:?}"),
                        }
                    }
                    Err(err) => warn!("failed to append chat message: {err:?}"),
                }
            }
        })
        .detach();
    }

    fn start_new_conversation(&self, cx: &mut Context<Self>) {
        let Some(services) = cx.try_global::<ChatServices>().cloned() else {
            return;
        };

        cx.spawn(async move |_weak: WeakEntity<Self>, app| {
            match services
                .create_conversation(DEFAULT_CHAT_TITLE, DEFAULT_CHAT_MODEL)
                .await
            {
                Ok(summary) => {
                    let summary_clone = summary.clone();
                    let _ = app.update(|app| {
                        let state = app.global::<ChatState>();
                        let conversations = state.conversations.clone();
                        let current = state.current_conversation.clone();
                        let messages = state.messages.clone();

                        conversations.update(app, |list, cx| {
                            list.insert(0, summary_clone);
                            cx.notify();
                        });
                        current.update(app, |slot, cx| {
                            *slot = Some(summary.id.clone());
                            cx.notify();
                        });
                        messages.update(app, |msgs, cx| {
                            msgs.clear();
                            cx.notify();
                        });
                    });
                }
                Err(err) => warn!("failed to create conversation: {err:?}"),
            }
        })
        .detach();
    }
}

impl Render for ChatOverview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if cx.try_global::<ChatServices>().is_none() {
            return div()
                .px(px(24.0))
                .py(px(16.0))
                .text_color(rgba(0x94a3b8ff))
                .child("聊天不可用：缺少 Turso 配置");
        }

        if !self.focus.is_focused(window) {
            self.focus.focus(window);
        }

        let state = cx.global::<ChatState>();
        let current_entity = state.current_conversation.clone();
        let current = state.current_conversation.read(cx).clone();
        let conversations = state.conversations.read(cx).clone();
        let messages = state.messages.read(cx).clone();

        let mut conversation_column = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .flex_basis(px(220.0))
            .flex_shrink_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().font_weight(FontWeight::BOLD).child("会话列表"))
                    .child(
                        button()
                            .style(ButtonStyle::MinimalNoRounding)
                            .size(ButtonSize::Regular)
                            .intent(ButtonIntent::Primary)
                            .child("+ 新建")
                            .id("chat-new-conversation")
                            .on_click(
                                cx.listener(|this, _, _, cx| this.start_new_conversation(cx)),
                            ),
                    ),
            );

        if conversations.is_empty() {
            conversation_column =
                conversation_column.child(div().text_color(rgba(0x94a3b8ff)).child("暂无会话"));
        } else {
            for (idx, conversation) in conversations.iter().enumerate() {
                let conversation_id: ConversationId = conversation.id.clone();
                let is_selected = current
                    .as_ref()
                    .map(|id| id == &conversation_id)
                    .unwrap_or(false);

                let mut item = div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .rounded(px(8.0))
                    .px(px(12.0))
                    .py(px(10.0))
                    .cursor(CursorStyle::PointingHand)
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(conversation.title.clone()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgba(0x94a3b8ff))
                            .child(format!("模型: {}", conversation.model_id)),
                    )
                    .hover(|div| div.bg(rgba(0x1b2434ff)));

                if is_selected {
                    item = item.bg(rgba(0x1f2937ff));
                }

                let current_handle = current_entity.clone();
                let conversation_for_selection = conversation_id.clone();
                let conversation_for_fetch = conversation_id.clone();

                let item = item.id(("chat-conversation", idx)).on_click(cx.listener(
                    move |_, _, _, cx| {
                        current_handle.update(cx, |slot, cx| {
                            *slot = Some(conversation_for_selection.clone());
                            cx.notify();
                        });

                        if let Some(services) = cx.try_global::<ChatServices>().cloned() {
                            chat::load_messages_for(cx, services, conversation_for_fetch.clone());
                        }
                    },
                ));

                conversation_column = conversation_column.child(item);
            }
        }

        let mut message_column = div().flex().flex_col().flex_grow().gap(px(12.0)).child(
            div().font_weight(FontWeight::BOLD).child(
                current
                    .as_ref()
                    .map(|id| format!("当前会话: {}", id.0))
                    .unwrap_or_else(|| "当前会话: 无".to_string()),
            ),
        );

        if messages.is_empty() {
            message_column =
                message_column.child(div().text_color(rgba(0x94a3b8ff)).child("该会话暂无消息"));
        } else {
            for message in messages.iter().take(100) {
                message_column = message_column.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .rounded(px(6.0))
                        .bg(rgba(0x111827ff))
                        .px(px(12.0))
                        .py(px(10.0))
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_sm()
                                .text_color(rgba(0x94a3b8ff))
                                .child(format!("{:?}", message.role)),
                        )
                        .child(
                            div()
                                .text_color(rgba(0xe2e8f0ff))
                                .child(message.content.clone()),
                        ),
                );
            }
        }

        let composer = div()
            .flex()
            .gap(px(12.0))
            .items_center()
            .pt(px(8.0))
            .border_t_1()
            .border_color(rgba(0x1f2937ff))
            .child(
                div()
                    .flex()
                    .flex_grow()
                    .bg(rgba(0x111827ff))
                    .rounded(px(8.0))
                    .px(px(10.0))
                    .py(px(8.0))
                    .child(self.input.clone()),
            )
            .child(
                button()
                    .intent(ButtonIntent::Primary)
                    .size(ButtonSize::Regular)
                    .child("发送")
                    .id("chat-send-message")
                    .on_click(cx.listener(|this, _, _, cx| this.submit_message(cx))),
            );

        div()
            .flex()
            .gap(px(24.0))
            .px(px(24.0))
            .py(px(16.0))
            .child(conversation_column)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .gap(px(16.0))
                    .child(message_column)
                    .child(composer),
            )
    }
}
