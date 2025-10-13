use std::time::SystemTime;

use gpui::{App, AppContext, Entity, Global};
use serde::{Deserialize, Serialize};

/// Unique conversation identifier.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConversationId(pub String);

impl ConversationId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Lightweight conversation summary for rendering lists.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: ConversationId,
    pub title: String,
    pub updated_at: SystemTime,
    pub model_id: String,
}

/// Individual chat message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: ConversationId,
    pub role: MessageRole,
    pub content: String,
    pub created_at: SystemTime,
    pub token_usage: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug)]
pub enum LlmRequestState {
    Idle,
    InFlight,
    Streaming { received_bytes: usize },
    Error(String),
}

impl Default for LlmRequestState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Root chat state registered as a gpui global model.
pub struct ChatState {
    pub conversations: Entity<Vec<ConversationSummary>>,
    pub current_conversation: Entity<Option<ConversationId>>,
    pub messages: Entity<Vec<Message>>,
    pub request_state: Entity<LlmRequestState>,
}

impl Global for ChatState {}

impl ChatState {
    pub fn register(cx: &mut App) {
        let conversations = cx.new(|_| Vec::<ConversationSummary>::new());
        let current_conversation = cx.new(|_| None::<ConversationId>);
        let messages = cx.new(|_| Vec::<Message>::new());
        let request_state = cx.new(|_| LlmRequestState::Idle);

        cx.set_global(ChatState {
            conversations,
            current_conversation,
            messages,
            request_state,
        });
    }
}
