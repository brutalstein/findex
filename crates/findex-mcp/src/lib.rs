//! Small MCP JSON-RPC stdio adapter. Each line is one UTF-8 JSON-RPC message.
//! No network listener, telemetry upload or credential interception.

use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

const MAX_REQUEST_BYTES: usize = 1024 * 1024;

pub fn serve(project: &Path) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = BufReader::new(stdin.lock());
    let mut output = stdout.lock();
    loop {
        let mut buffer = Vec::new();
        let count = input.read_until(b'\n', &mut buffer)?;
        if count == 0 { break; }
        if buffer.len() > MAX_REQUEST_BYTES {
            // Protocol has no safe way to recover from unbounded requests: fail closed.
            return Err(io::Error::new(io::ErrorKind::InvalidData, "MCP request exceeded 1 MiB"));
        }
        let request: Value = match serde_json::from_slice(&buffer) {
            Ok(value) => value,
            Err(_) => {
                send(&mut output, &error(Value::Null, -32700, "invalid JSON"))?;
                continue;
            }
        };
        if let Some(response) = handle(&request, project) {
            send(&mut output, &response)?;
        }
    }
    Ok(())
}

fn send(output: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, value)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    output.write_all(b"\n")?;
    output.flush()
}

fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

fn success(id: Value, value: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":value})
}

/// Pure protocol dispatch, except for the intentionally bounded tool invocation.
/// Unknown notifications are ignored; calls receive JSON-RPC errors.
pub fn handle(request: &Value, project: &Path) -> Option<Value> {
    let method = request.get("method")?.as_str()?;
    let id = request.get("id")?.clone();
    let response = match method {
        "initialize" => {
            let requested = request.pointer("/params/protocolVersion").and_then(Value::as_str);
            let version = match requested {
                Some("2025-11-25") => "2025-11-25",
                Some("2025-06-18") => "2025-06-18",
                Some("2024-11-05") => "2024-11-05",
                _ => "2025-06-18",
            };
            success(id, json!({
                "protocolVersion": version,
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name":"findex", "version":env!("CARGO_PKG_VERSION")},
                "instructions": "Use findex_search for lexical evidence-based code discovery. Results are not proof of correctness; verify before editing."
            }))
        }
        "ping" => success(id, json!({})),
        "tools/list" => success(id, json!({
            "tools": [{
                "name": "findex_search",
                "description": "Search the current local project by identifiers and code terms. Returns verified file/line evidence. Keyword/BM25 only in v0.1, not semantic or graph search.",
                "inputSchema": {
                    "type":"object",
                    "properties":{
                        "query":{"type":"string","minLength":1,"maxLength":500},
                        "limit":{"type":"integer","minimum":1,"maximum":32,"default":8}
                    },
                    "required":["query"],
                    "additionalProperties":false
                }
            }]
        })),
        "tools/call" => {
            let name = request.pointer("/params/name").and_then(Value::as_str);
            if name != Some("findex_search") {
                error(id, -32602, "unknown tool")
            } else {
                let args = request.pointer("/params/arguments");
                let query = args.and_then(|v|v.get("query")).and_then(Value::as_str).unwrap_or("");
                let limit = args.and_then(|v|v.get("limit")).and_then(Value::as_u64).unwrap_or(8);
                if query.is_empty() || query.len() > 500 || limit == 0 || limit > 32 {
                    error(id, -32602, "query must be 1-500 bytes and limit 1-32")
                } else {
                    match findex_index::search(project, query, limit as usize) {
                        Ok(found) => {
                            let payload = serde_json::to_string(&found).unwrap_or_else(|_| "{}".to_string());
                            success(id, json!({"content":[{"type":"text", "text":payload}], "isError":false}))
                        }
                        Err(err) => success(id, json!({"content":[{"type":"text", "text":format!("Findex index unavailable: {err}")}], "isError":true}))
                    }
                }
            }
        }
        _ => error(id, -32601, "method not found"),
    };
    Some(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initialization_and_tool_discovery() {
        let project = Path::new(".");
        let init = handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}), project).expect("init");
        assert_eq!(init["result"]["serverInfo"]["name"], "findex");
        let tools = handle(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}), project).expect("tools");
        assert_eq!(tools["result"]["tools"][0]["name"], "findex_search");
    }
    #[test]
    fn notifications_produce_no_response() {
        assert!(handle(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}), Path::new(".")).is_none());
    }
    #[test]
    fn disallows_unrecognized_tool_and_unbounded_limits() {
        let project = Path::new(".");
        let response = handle(&json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"other","arguments":{}}}), project).expect("response");
        assert_eq!(response["error"]["code"], -32602);
        let oversized = handle(&json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"findex_search","arguments":{"query":"abc","limit":1000000}}}), project).expect("response");
        assert_eq!(oversized["error"]["code"], -32602);
    }
}
