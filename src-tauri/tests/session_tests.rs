//! Session management tests

use tempfile::TempDir;
use ssk_lib::{Session, ConversationMessage, MessageRole, ContentBlock};

#[test]
fn test_session_new() {
    let session = Session::new();
    assert!(!session.id.is_empty());
    assert!(session.messages.is_empty());
    assert!(session.file_path.is_none());
}

#[test]
fn test_session_new_with_path() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("test_session.jsonl");

    let session = Session::new_with_path(&path);
    assert!(!session.id.is_empty());
    assert!(session.messages.is_empty());
    assert!(path.exists());
}

#[test]
fn test_session_push_user_text() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("test_session.jsonl");

    let mut session = Session::new_with_path(&path);
    session.push_user_text("Hello, world!");

    assert_eq!(session.messages.len(), 1);
    let msg = &session.messages[0];
    assert!(matches!(msg.role, MessageRole::User));
    assert_eq!(msg.text(), "Hello, world!");
}

#[test]
fn test_session_message_text() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("test_session.jsonl");

    let mut session = Session::new_with_path(&path);
    session.push_user_text("Line 1\nLine 2");

    assert_eq!(session.messages[0].text(), "Line 1\nLine 2");
}

#[test]
fn test_session_clear() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("test_session.jsonl");

    let mut session = Session::new_with_path(&path);
    session.push_user_text("Hello");
    assert_eq!(session.messages.len(), 1);

    session.clear();
    assert!(session.messages.is_empty());
}

#[test]
fn test_session_record_compaction() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("test_session.jsonl");

    let mut session = Session::new_with_path(&path);

    assert!(session.compaction.is_none());

    session.record_compaction("Summarized 5 messages", 5);

    let compaction = session.compaction.clone().unwrap();
    assert_eq!(compaction.count, 1);
    assert_eq!(compaction.removed_message_count, 5);
    assert_eq!(compaction.summary, "Summarized 5 messages");
}

#[test]
fn test_session_record_multiple_compactions() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("test_session.jsonl");

    let mut session = Session::new_with_path(&path);

    session.record_compaction("First compaction", 3);
    session.record_compaction("Second compaction", 5);

    let compaction = session.compaction.clone().unwrap();
    assert_eq!(compaction.count, 2);
    assert_eq!(compaction.removed_message_count, 5);
}

#[test]
fn test_session_clone() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("test_session.jsonl");

    let mut session = Session::new_with_path(&path);
    session.push_user_text("Hello");

    let cloned = session.clone();
    assert_eq!(cloned.id, session.id);
    assert_eq!(cloned.messages.len(), session.messages.len());
    // Writer should not be cloned
    assert!(cloned.writer.is_none());
}

#[test]
fn test_conversation_message_user_text() {
    let msg = ConversationMessage::user_text("Test");
    assert!(matches!(msg.role, MessageRole::User));
    assert_eq!(msg.text(), "Test");
}

#[test]
fn test_conversation_message_assistant() {
    let blocks = vec![ContentBlock::Text {
        text: "Hello".to_string(),
    }];
    let msg = ConversationMessage::assistant(blocks);
    assert!(matches!(msg.role, MessageRole::Assistant));
    assert_eq!(msg.text(), "Hello");
}

#[test]
fn test_conversation_message_tool_result() {
    let msg = ConversationMessage::tool_result(
        "tool_123",
        "read",
        "file content",
        false,
    );
    assert!(matches!(msg.role, MessageRole::Tool));

    let blocks = &msg.blocks;
    assert_eq!(blocks.len(), 1);
    if let ContentBlock::ToolResult { tool_use_id, tool_name, output, is_error } = &blocks[0] {
        assert_eq!(tool_use_id, "tool_123");
        assert_eq!(tool_name, "read");
        assert_eq!(output, "file content");
        assert!(!*is_error);
    } else {
        panic!("Expected ToolResult block");
    }
}

#[test]
fn test_message_role_as_str() {
    assert_eq!(MessageRole::System.as_str(), "system");
    assert_eq!(MessageRole::User.as_str(), "user");
    assert_eq!(MessageRole::Assistant.as_str(), "assistant");
    assert_eq!(MessageRole::Tool.as_str(), "tool");
}

#[test]
fn test_session_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("save_test.jsonl");

    // Create session with messages
    {
        let mut session = Session::new_with_path(&path);
        session.push_user_text("Message 1");
        session.push_user_text("Message 2");
        session.save().unwrap();
    }

    // Load session from file
    let loaded = Session::load_from_file(&path).unwrap();
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.messages[0].text(), "Message 1");
    assert_eq!(loaded.messages[1].text(), "Message 2");
}

#[test]
fn test_session_default() {
    let session = Session::default();
    assert!(!session.id.is_empty());
    assert!(session.messages.is_empty());
}
