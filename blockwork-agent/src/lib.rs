//! `blockwork-agent`: the CLI and MCP server for Blockwork automation
//! (see `docs/agent.md`). One binary, one tool table (`tools.rs`), two
//! surfaces (CLI + MCP over stdio).

pub mod client;
pub mod mcp;
pub mod tools;

pub use client::{AgentError, DaemonClient};
pub use tools::{ToolDef, tool_table};
