class Omni < Formula
  desc "Semantic Signal Engine — Less noise. More signal. Right signal."
  homepage "https://github.com/fajarhide/omni"
  version "0.5.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "87f8bd502e2161f12eb0d671c3dc0748f751750fb02f348d62207266a8323d98"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "3871d49daadbe598c9f2b8c2d750da9f780ae411d4be093a6cfa7532fb5ef863"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "0a0d54fb4c702b9eeeea7d32f47a08f8933cd766896e989117ff14235da91dba"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "2438e14e924a7207f07ee6bf530a05c5d89cb7e7ca87f224bc11a476978fd903"
    end
  end

  def install
    bin.install "omni"
  end

  def caveats
    <<~EOS
      Quick start:
        omni init --hook   # Activate PostToolUse hook for Claude Code
        omni init --mcp    # Setup MCP for Claude Desktop
        omni doctor        # Verify installation
        omni stats         # View token savings

      OMNI works automatically — no configuration needed.
      Hooks intercept Claude Code tool outputs and distill them in real-time.
    EOS
  end

  test do
    assert_match "omni", shell_output("#{bin}/omni version")
    assert_match "Signal Report", shell_output("#{bin}/omni stats 2>&1", 0)
    assert_match "OMNI Doctor", shell_output("#{bin}/omni doctor 2>&1", 0)
  end
end
