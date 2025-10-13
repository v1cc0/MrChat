use gpui::{div, px, FontWeight, IntoElement, ParentElement, Render, Styled, Window};

use crate::modules::chat::models::{ChatState, ConversationSummary, Message};

pub struct ChatOverview;

impl ChatOverview {
    pub fn new() -> Self {
        Self
    }

    fn render_conversations(&self, conversations: &[ConversationSummary]) -> Vec<gpui::AnyElement> {
        if conversations.is_empty() {
            return vec![
                div()
                    .text_color(gpui::rgba(0x94a3b8ff))
                    .child("暂无会话")
                    .into_any_element(),
            ];
        }

        conversations
            .iter()
            .map(|conversation| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(conversation.title.clone()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(gpui::rgba(0x94a3b8ff))
                            .child(format!("模型: {}", conversation.model_id)),
                    )
                    .into_any_element()
            })
            .collect()
    }

    fn render_messages(&self, messages: &[Message]) -> Vec<gpui::AnyElement> {
        if messages.is_empty() {
            return vec![
                div()
                    .text_color(gpui::rgba(0x94a3b8ff))
                    .child("该会话暂无消息")
                    .into_any_element(),
            ];
        }

        messages
            .iter()
            .map(|message| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_sm()
                            .child(format!("{:?}", message.role)),
                    )
                    .child(
                        div()
                            .text_color(gpui::rgba(0xe2e8f0ff))
                            .child(message.content.clone()),
                    )
                    .into_any_element()
            })
            .collect()
    }
}

impl Render for ChatOverview {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let state = cx.global::<ChatState>();
        let conversations = state.conversations.read(cx).clone();
        let messages = state.messages.read(cx).clone();
        let current = state.current_conversation.read(cx).clone();

        let conversation_column = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(div().font_weight(gpui::FontWeight::BOLD).child("会话列表"))
            .children(self.render_conversations(&conversations));

        let message_column = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div().font_weight(gpui::FontWeight::BOLD).child(
                    current
                        .map(|id| format!("当前会话: {}", id.0))
                        .unwrap_or_else(|| "当前会话: 无".to_string()),
                ),
            )
            .children(self.render_messages(&messages));

        div()
            .flex()
            .gap(px(24.0))
            .px(px(24.0))
            .py(px(16.0))
            .child(conversation_column)
            .child(message_column)
    }
}
