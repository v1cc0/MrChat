use gpui::{
    CursorStyle, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px, rgba,
};

use crate::modules::chat::{self, models::ChatState, services::ChatServices};

pub struct ChatOverview;

impl ChatOverview {
    pub fn new() -> Self {
        Self
    }
}

impl Render for ChatOverview {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let state = cx.global::<ChatState>();
        let current_entity = state.current_conversation.clone();
        let conversations = state.conversations.read(cx).clone();
        let messages = state.messages.read(cx).clone();
        let current = state.current_conversation.read(cx).clone();

        let mut conversation_column = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .flex_basis(px(220.0))
            .flex_shrink_0()
            .child(div().font_weight(FontWeight::BOLD).child("会话列表"));

        if conversations.is_empty() {
            conversation_column =
                conversation_column.child(div().text_color(rgba(0x94a3b8ff)).child("暂无会话"));
        } else {
            for (idx, conversation) in conversations.iter().enumerate() {
                let conversation_id = conversation.id.clone();
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
            for message in messages.iter().take(50) {
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

        div()
            .flex()
            .gap(px(24.0))
            .px(px(24.0))
            .py(px(16.0))
            .child(conversation_column)
            .child(message_column)
    }
}
