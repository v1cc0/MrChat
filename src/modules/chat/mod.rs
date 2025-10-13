pub mod models;
pub mod services;
pub mod storage;
pub mod ui;

use std::sync::Arc;

use anyhow::Result;
use gpui::App;
use tracing::error;

use crate::{config::ChatSection, shared::db::TursoPool};

use self::{
    models::{ChatState, ConversationId, LlmRequestState},
    services::ChatServices,
    storage::ChatDao,
};

/// High level entry point for chat functionality.
pub struct ChatFacade {
    db: Arc<TursoPool>,
    chat_config: ChatSection,
    api_key: Option<String>,
}

impl ChatFacade {
    pub fn new(db: Arc<TursoPool>, chat_config: ChatSection, api_key: Option<String>) -> Self {
        Self {
            db,
            chat_config,
            api_key,
        }
    }

    pub fn db(&self) -> Arc<TursoPool> {
        Arc::clone(&self.db)
    }

    pub fn services(&self) -> services::ChatServices {
        services::ChatServices::new(
            self.db.clone(),
            self.chat_config.clone(),
            self.api_key.clone(),
        )
    }

    pub fn dao(&self) -> ChatDao {
        ChatDao::new(self.db.clone())
    }
}

/// Bootstrap helper used when we only have configuration yet.
pub async fn init_from_pool(
    pool: TursoPool,
    chat_config: ChatSection,
    api_key: Option<String>,
) -> Result<ChatFacade> {
    Ok(ChatFacade::new(Arc::new(pool), chat_config, api_key))
}

pub fn ensure_state_registered(cx: &mut App) {
    if !cx.has_global::<ChatState>() {
        ChatState::register(cx);
    }
}

pub fn bootstrap_state(cx: &mut App, services: ChatServices) {
    let (conversations, current, messages, request_state) = {
        let state = cx.global::<ChatState>();
        (
            state.conversations.clone(),
            state.current_conversation.clone(),
            state.messages.clone(),
            state.request_state.clone(),
        )
    };

    cx.spawn(async move |app| {
        if let Err(err) = services.ensure_schema().await {
            error!("failed to ensure chat schema: {err:?}");
            return;
        }

        match services.list_conversations().await {
            Ok(conversation_list) => {
                let first_conversation = conversation_list.first().cloned();
                let selected_id = first_conversation.as_ref().map(|c| c.id.clone());

                if let Err(err) = app.update(|app| {
                    conversations.update(app, |data, cx| {
                        *data = conversation_list.clone();
                        cx.notify();
                    });
                    current.update(app, |slot, cx| {
                        *slot = selected_id.clone();
                        cx.notify();
                    });
                    messages.update(app, |data, cx| {
                        data.clear();
                        cx.notify();
                    });
                }) {
                    error!("failed to populate chat conversations: {err:?}");
                    return;
                }

                if let Some(conversation_id) = selected_id {
                    match services.list_messages(&conversation_id).await {
                        Ok(history) => {
                            if let Err(err) = app.update(|app| {
                                messages.update(app, |data, cx| {
                                    *data = history;
                                    cx.notify();
                                });
                            }) {
                                error!("failed to populate initial messages: {err:?}");
                            }
                        }
                        Err(err) => error!("failed to fetch conversation history: {err:?}"),
                    }
                }
            }
            Err(err) => error!("failed to load conversations: {err:?}"),
        }

        let _ = app.update(|app| {
            request_state.update(app, |slot, cx| {
                *slot = LlmRequestState::Idle;
                cx.notify();
            });
        });
    })
    .detach();
}

pub fn load_messages_for(cx: &mut App, services: ChatServices, conversation_id: ConversationId) {
    let (messages, current) = {
        let state = cx.global::<ChatState>();
        (state.messages.clone(), state.current_conversation.clone())
    };

    cx.spawn(
        async move |app| match services.list_messages(&conversation_id).await {
            Ok(history) => {
                let conversation_id_clone = conversation_id.clone();
                let _ = app.update(|app| {
                    current.update(app, |slot, cx| {
                        *slot = Some(conversation_id_clone.clone());
                        cx.notify();
                    });
                    messages.update(app, |data, cx| {
                        *data = history;
                        cx.notify();
                    });
                });
            }
            Err(err) => error!("failed to load messages: {err:?}"),
        },
    )
    .detach();
}
