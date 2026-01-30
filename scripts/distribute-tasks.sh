#!/bin/bash
# Distribute tasks from openspec/changes/bend-rs/tasks.md to domain-specific task files
#
# Usage: ./scripts/distribute-tasks.sh
#
# This script reads the master task list and categorizes tasks into:
# - tasks/backend.md (core logic, data structures, parsing)
# - tasks/frontend.md (UI, user interaction)
# - tasks/infrastructure.md (build, test, release)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
MASTER_TASKS="$PROJECT_ROOT/openspec/changes/bend-rs/tasks.md"
TASKS_DIR="$PROJECT_ROOT/tasks"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "Task Distribution Script"
echo "========================"
echo ""

if [[ ! -f "$MASTER_TASKS" ]]; then
    echo -e "${RED}Error: Master task file not found at $MASTER_TASKS${NC}"
    exit 1
fi

# Count tasks in each file
count_tasks() {
    local file=$1
    grep -c '^\- \[ \]' "$file" 2>/dev/null || echo 0
}

echo "Current task counts:"
echo "  Backend:        $(count_tasks "$TASKS_DIR/backend.md") pending"
echo "  Frontend:       $(count_tasks "$TASKS_DIR/frontend.md") pending"
echo "  Infrastructure: $(count_tasks "$TASKS_DIR/infrastructure.md") pending"
echo ""

# Count master tasks
master_count=$(count_tasks "$MASTER_TASKS")
echo "Master task file: $master_count total tasks"
echo ""

# Show distribution summary
echo -e "${GREEN}Task files are located in: $TASKS_DIR/${NC}"
echo ""
echo "To manually add a task, edit the appropriate file and add:"
echo '  - [ ] PHASE.NUMBER Description of task'
echo ""
echo "Example:"
echo '  - [ ] 99.1 Add unit tests for BMP parser'
