#!/bin/bash
# omni-deploy-edge.sh
# Developer Velocity. Focus on "Build once, run anywhere".

set -euo pipefail

BLUE='\033[0;34m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${BLUE}🚢 OMNI Edge Deployment Preparer${NC}"
echo "════════════════════════════════════════════════"

echo -e "${CYAN}Step 1: Building Native Core...${NC}"
(cd core && zig build -Doptimize=ReleaseFast -Dversion=0.4.3 -p ../)

echo -e "${CYAN}Step 2: Building WebAssembly Binary (Edge)...${NC}"
(cd core && zig build wasm -Doptimize=ReleaseSmall -Dversion=0.4.3 -p ../)

echo -e "${CYAN}Step 3: Building MCP Server...${NC}"
npm run build

echo -e "${CYAN}Step 4: Verifying Binaries...${NC}"
if [ -f "bin/omni" ] && [ -f "bin/omni-wasm.wasm" ]; then
    echo -e "${GREEN}✅ OMNI Binaries Ready.${NC}"
else
    echo -e "${RED}❌ Build Failed${NC}"
    exit 1
fi

echo -e "\n${GREEN} OMNI is ready for Edge Deployment!${NC}"
echo "Run 'bin/omni monitor' to verify system health."
