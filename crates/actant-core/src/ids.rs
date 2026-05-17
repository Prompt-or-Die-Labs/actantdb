//! Identifier newtypes. Every ActantDB row id is a stringly-typed ULID.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! id_type {
    ($name:ident, $prefix:expr) => {
        #[doc = concat!("Identifier for ", stringify!($name), ". Wraps a ULID string.")]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// Generate a new id with the configured prefix.
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, ulid::Ulid::new()))
            }
            /// Wrap an existing string.
            pub fn from_string(s: impl Into<String>) -> Self {
                Self(s.into())
            }
            /// Borrow as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

id_type!(WorkspaceId, "ws");
id_type!(ActorId, "act");
id_type!(SessionId, "sess");
id_type!(MessageId, "msg");
id_type!(EventId, "evt");
id_type!(CommandId, "cmd");
id_type!(ModelRouteId, "route");
id_type!(ModelProviderId, "mp");
id_type!(ModelCallId, "mc");
id_type!(ContextBuildId, "ctx");
id_type!(ContextItemId, "ci");
id_type!(ToolId, "tool");
id_type!(ToolCallId, "tc");
id_type!(EffectId, "eff");
id_type!(WorkerId, "wrk");
id_type!(ApprovalRequestId, "appr");
id_type!(AuthorityScopeId, "as");
id_type!(MemoryId, "mem");
id_type!(MemoryCandidateId, "memc");
id_type!(ArtifactId, "art");
id_type!(PolicyId, "pol");
id_type!(WorkflowId, "wf");
id_type!(WorkflowNodeId, "wfn");
id_type!(WorkflowRunId, "wfr");
id_type!(ReplayCheckpointId, "chk");
id_type!(ReplayRunId, "rr");
id_type!(TriggerId, "trg");
id_type!(EmbeddingRefId, "emb");
id_type!(SecretRefId, "sec");
