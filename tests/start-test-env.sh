#!/bin/bash

# E2Eテスト用の開発環境起動スクリプト
# バックエンドとフロントエンドを起動し、ヘルスチェックを行う

set -e

# カラー出力
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# 設定
BACKEND_URL="http://localhost:8000"
FRONTEND_URL="http://localhost:5173"
BACKEND_HEALTH_ENDPOINT="$BACKEND_URL/health"
MAX_WAIT_TIME=60  # 最大待機時間（秒）

# ヘルプメッセージ
show_help() {
    echo "使用法: $0 [オプション]"
    echo ""
    echo "オプション:"
    echo "  -h, --help          このヘルプを表示"
    echo "  -w, --wait-only     既に起動している環境のヘルスチェックのみを実行"
    echo "  -c, --cleanup       既存のプロセスをクリーンアップしてから起動"
    echo ""
    echo "説明:"
    echo "  E2Eテスト用にバックエンドとフロントエンドを起動します。"
    echo "  バックエンド: $BACKEND_URL"
    echo "  フロントエンド: $FRONTEND_URL"
    echo ""
    echo "  停止するには Ctrl+C を押してください。"
}

# 既存プロセスのクリーンアップ
cleanup_processes() {
    echo -e "${YELLOW}既存のプロセスをクリーンアップしています...${NC}"

    # バックエンドプロセスを終了
    if lsof -i:8000 -t >/dev/null 2>&1; then
        echo -e "${YELLOW}ポート8000のプロセスを終了しています...${NC}"
        lsof -i:8000 -t | xargs kill -9 2>/dev/null || true
    fi

    # フロントエンドプロセスを終了
    if lsof -i:5173 -t >/dev/null 2>&1; then
        echo -e "${YELLOW}ポート5173のプロセスを終了しています...${NC}"
        lsof -i:5173 -t | xargs kill -9 2>/dev/null || true
    fi

    sleep 2
    echo -e "${GREEN}クリーンアップ完了${NC}"
}

# ヘルスチェック
check_health() {
    local service_name=$1
    local url=$2
    local max_attempts=$((MAX_WAIT_TIME / 2))
    local attempt=1

    echo -e "${BLUE}$service_name のヘルスチェック中...${NC}"

    while [ $attempt -le $max_attempts ]; do
        if curl -s -o /dev/null -w "%{http_code}" "$url" | grep -q "200\|404"; then
            echo -e "${GREEN}✓ $service_name が起動しました (${attempt}回目の試行)${NC}"
            return 0
        fi

        echo -e "${YELLOW}  待機中... (${attempt}/${max_attempts})${NC}"
        sleep 2
        ((attempt++))
    done

    echo -e "${RED}✗ $service_name の起動に失敗しました (タイムアウト)${NC}"
    return 1
}

# オプション解析
WAIT_ONLY=false
CLEANUP=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        -w|--wait-only)
            WAIT_ONLY=true
            shift
            ;;
        -c|--cleanup)
            CLEANUP=true
            shift
            ;;
        *)
            echo -e "${RED}エラー: 不明なオプション $1${NC}"
            show_help
            exit 1
            ;;
    esac
done

# クリーンアップが要求された場合
if [ "$CLEANUP" = true ]; then
    cleanup_processes
fi

# 既に起動している環境のヘルスチェックのみを実行
if [ "$WAIT_ONLY" = true ]; then
    echo -e "${BLUE}既存の環境のヘルスチェックを実行しています...${NC}"

    if check_health "バックエンド" "$FRONTEND_URL" && \
       check_health "フロントエンド" "$FRONTEND_URL"; then
        echo -e "${GREEN}すべてのサービスが正常に動作しています${NC}"
        exit 0
    else
        echo -e "${RED}一部のサービスが起動していません${NC}"
        exit 1
    fi
fi

# メインの起動処理
echo -e "${BLUE}E2Eテスト環境を起動しています...${NC}"
echo -e "${BLUE}バックエンド: $BACKEND_URL${NC}"
echo -e "${BLUE}フロントエンド: $FRONTEND_URL${NC}"
echo -e "${BLUE}停止するには Ctrl+C を押してください${NC}"
echo ""

# プロセス終了時に子プロセスも終了させる
trap 'echo -e "\n${YELLOW}プロセスを終了しています...${NC}"; kill 0' EXIT INT TERM

# バックエンドを起動
echo -e "${GREEN}[Backend]${NC} 起動中..."
(
    cd "$(dirname "$0")/../backend"
    source ~/.cargo/env 2>/dev/null || true
    cargo run 2>&1 | while IFS= read -r line; do
        echo -e "${GREEN}[Backend]${NC} $line"
    done
) &
BACKEND_PID=$!

# バックエンドのヘルスチェック
if ! check_health "バックエンド" "$FRONTEND_URL"; then
    echo -e "${RED}バックエンドの起動に失敗しました${NC}"
    kill $BACKEND_PID 2>/dev/null || true
    exit 1
fi

# フロントエンドを起動
echo -e "${GREEN}[Frontend]${NC} 起動中..."
(
    cd "$(dirname "$0")/../frontend"
    npm run dev 2>&1 | while IFS= read -r line; do
        echo -e "${BLUE}[Frontend]${NC} $line"
    done
) &
FRONTEND_PID=$!

# フロントエンドのヘルスチェック
if ! check_health "フロントエンド" "$FRONTEND_URL"; then
    echo -e "${RED}フロントエンドの起動に失敗しました${NC}"
    kill $BACKEND_PID $FRONTEND_PID 2>/dev/null || true
    exit 1
fi

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}E2Eテスト環境が正常に起動しました！${NC}"
echo -e "${GREEN}========================================${NC}"
echo -e "${BLUE}バックエンド: $BACKEND_URL${NC}"
echo -e "${BLUE}フロントエンド: $FRONTEND_URL${NC}"
echo ""
echo -e "${YELLOW}E2Eテストを実行する準備が整いました。${NC}"
echo -e "${YELLOW}停止するには Ctrl+C を押してください。${NC}"
echo ""

# 両方のプロセスが終了するまで待機
wait
