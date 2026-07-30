pub mod doubao;

pub use doubao::{
    is_doubao_provider, recognize_doubao_with_transcript, test_doubao_connection, DoubaoAuthMode,
    DoubaoProviderSettings, DEFAULT_DOUBAO_ENDPOINT, DEFAULT_DOUBAO_RESOURCE_ID, KIND_DOUBAO_WS,
    KIND_OPENAI_COMPATIBLE,
};
