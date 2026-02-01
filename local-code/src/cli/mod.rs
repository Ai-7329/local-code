pub mod repl;
pub mod commands;
pub mod output;
pub mod spinner;
pub mod completion;
pub mod confirm;
pub mod ui;

pub use repl::Repl;
pub use commands::{Command, CommandHandler, CommandResult};
pub use output::{
    print_error, print_success, print_tool, print_mode, print_info, print_banner,
    print_startup_banner,
    StreamingWriter, print_streaming_start, print_streaming_text,
    print_streaming_end, print_streaming_end_with_stats,
    OutputPostProcessor,
};
pub use spinner::Spinner;
pub use completion::{Completer, CompletionResult};
pub use confirm::{ConfirmDialog, ConfirmResult, confirm, confirm_tool_execution, requires_confirmation};
pub use ui::{
    Ui, StatusLine,
    print_separator, print_formatted_block, print_processing,
    print_error as ui_print_error, print_info as ui_print_info,
};
