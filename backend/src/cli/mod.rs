pub mod commands;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "tfkosmos",
    about = "TFKosmos - Cloud Infrastructure to Terraform Code Generator",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// HTTPサーバーを起動（Pro以上で利用可能）
    #[cfg(feature = "gui-server")]
    Serve {
        #[arg(long, default_value = "0.0.0.0:8000")]
        bind: String,
    },
    /// クラウドリソースをスキャン
    Scan {
        /// スキャン設定ファイルパス（TOML or JSON）
        #[arg(short, long)]
        config: PathBuf,
        /// 出力フォーマット
        #[arg(short, long, default_value = "json")]
        output: OutputFormat,
    },
    /// Terraformコードを生成
    Generate {
        /// スキャンID
        #[arg(long)]
        scan_id: String,
        /// 出力ディレクトリ
        #[arg(long, default_value = "./terraform-output")]
        output_dir: PathBuf,
    },
    /// ドリフト検出
    Drift {
        /// スキャンID
        #[arg(long)]
        scan_id: String,
        /// Terraform stateファイルパス
        #[arg(long)]
        state_file: PathBuf,
        /// 出力フォーマット
        #[arg(short, long, default_value = "json")]
        output: OutputFormat,
    },
    /// ライセンス管理
    #[cfg(feature = "license-manager")]
    License {
        #[command(subcommand)]
        action: LicenseAction,
    },
}

#[derive(Subcommand)]
pub enum LicenseAction {
    /// ライセンスキーを登録してマシンを紐付け
    Activate {
        /// ライセンスキー
        key: String,
    },
    /// 現在のライセンス状態を表示
    Status,
    /// マシン紐付けを解除（マシン移行時）
    Deactivate,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Json,
    Table,
    Quiet,
}
