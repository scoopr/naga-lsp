//! A minimal example LSP server that can only respond to the `gotoDefinition` request. To use
//! this example, execute it and then send an `initialize` request.
//!
//! ```no_run
//! Content-Length: 85
//!
//! {"jsonrpc": "2.0", "method": "initialize", "id": 1, "params": {"capabilities": {}}}
//! ```
//!
//! This will respond with a server response. Then send it a `initialized` notification which will
//! have no response.
//!
//! ```no_run
//! Content-Length: 59
//!
//! {"jsonrpc": "2.0", "method": "initialized", "params": {}}
//! ```
//!
//! Once these two are sent, then we enter the main loop of the server. The only request this
//! example can handle is `gotoDefinition`:
//!
//! ```no_run
//! Content-Length: 159
//!
//! {"jsonrpc": "2.0", "method": "textDocument/definition", "id": 2, "params": {"textDocument": {"uri": "file://temp"}, "position": {"line": 1, "character": 1}}}
//! ```
//!
//! To finish up without errors, send a shutdown request:
//!
//! ```no_run
//! Content-Length: 67
//!
//! {"jsonrpc": "2.0", "method": "shutdown", "id": 3, "params": null}
//! ```
//!
//! The server will exit the main loop and finally we send a `shutdown` notification to stop
//! the server.
//!
//! ```
//! Content-Length: 54
//!
//! {"jsonrpc": "2.0", "method": "exit", "params": null}
//! ```
use std::error::Error;

use lsp_types::{
    notification::DidChangeTextDocument, Diagnostic, InitializeParams, PublishDiagnosticsParams,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
    VersionedTextDocumentIdentifier,
};

use lsp_server::{Connection, Message, Notification};

use naga::front::wgsl;

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    // Note that  we must have our logging only write out to stderr.
    eprintln!("starting generic LSP server");

    // Create the transport. Includes the stdio (stdin and stdout) versions but this could
    // also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();

    // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
    let mut server_caps = ServerCapabilities::default();
    server_caps.text_document_sync =
        Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::Full));

    let server_capabilities = serde_json::to_value(&server_caps).unwrap();
    let initialization_params = connection.initialize(server_capabilities)?;
    main_loop(&connection, initialization_params)?;
    io_threads.join()?;

    // Shut down gracefully.
    eprintln!("shutting down server");
    Ok(())
}

fn main_loop(
    connection: &Connection,
    params: serde_json::Value,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let _params: InitializeParams = serde_json::from_value(params).unwrap();
    eprintln!("starting example main loop");
    for msg in &connection.receiver {
        eprintln!("got msg: {:?}", msg);
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                // eprintln!("got request: {:?}", req);

                /*                 match cast::<GotoDefinition>(req) {
                    Ok((id, params)) => {
                        eprintln!("got gotoDefinition request #{}: {:?}", id, params);
                        let result = Some(GotoDefinitionResponse::Array(Vec::new()));
                        let result = serde_json::to_value(&result).unwrap();
                        let resp = Response { id, result: Some(result), error: None };
                        connection.sender.send(Message::Response(resp))?;
                        continue;
                    }
                    Err(req) => req,
                };*/
                // ...
            }
            Message::Response(_resp) => {
                // eprintln!("got response: {:?}", resp);
            }
            Message::Notification(not) => {
                // eprintln!("got notification: {:?}", not);

                if let Ok(did_change) = cast_notification::<DidChangeTextDocument>(not) {
                    // eprintln!("didChange {:?}", did_change);

                    // we are in full sync, so assume only one
                    let change = &did_change.content_changes[0];

                    let text = &change.text;

                    let res = wgsl::parse_str(text);
                    let mut diags = Vec::new();

                    match res {
                        Ok(_) => {}
                        Err(err) => {
                            eprint!("compile err: {:?}", err);

                            // let result = Some(Diagno);
                            // let result = serde_json::to_value(&result).unwrap();
                            // let resp = Response { id, result: Some(result), error: None };
                            let diag = Diagnostic {
                                range: lsp_types::Range {
                                    start: lsp_types::Position {
                                        line: err.pos.0 as u32 - 1,
                                        character: err.pos.1 as u32 - 1,
                                    },
                                    end: lsp_types::Position {
                                        line: err.pos.0 as u32 - 1,
                                        character: err.pos.1 as u32 + 99, // TODO,
                                    },
                                },
                                severity: Some(lsp_types::DiagnosticSeverity::Error),
                                code: None,
                                code_description: None,
                                source: None,
                                message: format!("{:?}", err),
                                related_information: None,
                                tags: None,
                                data: None,
                            };
                            diags.push(diag);
                        }
                    }
                    send_diagnostics(connection, did_change.text_document, diags)?;
                }
            }
        }
    }
    Ok(())
}

// fn cast<R>(req: Request) -> Result<(RequestId, R::Params), Request>
// where
//     R: lsp_types::request::Request,
//     R::Params: serde::de::DeserializeOwned,
// {
//     req.extract(R::METHOD)
// }

fn send_diagnostics(
    connection: &Connection,
    text_document: VersionedTextDocumentIdentifier,
    diags: Vec<Diagnostic>,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let pubdiag_params = PublishDiagnosticsParams {
        uri: text_document.uri,
        diagnostics: diags,
        version: Some(text_document.version),
    };
    let pubdiag_json = serde_json::to_value(&pubdiag_params).unwrap();
    let diag_not = Notification {
        method: "textDocument/publishDiagnostics".to_string(),
        params: pubdiag_json,
    };
    connection
        .sender
        .send(Message::Notification(diag_not))
        .map_err(Box::new)?;
    Ok(())
}

fn cast_notification<N>(not: Notification) -> Result<N::Params, Notification>
where
    N: lsp_types::notification::Notification,
    N::Params: serde::de::DeserializeOwned,
{
    not.extract(N::METHOD)
}
