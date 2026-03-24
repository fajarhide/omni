class Omni < Formula
  desc "Semantic Signal Engine — Less noise. More signal. Right signal."
  homepage "https://github.com/fajarhide/omni"
  version "0.5.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "9ac454110f4771c57e2a6196de2fd052bfa0941dfa3820d67dc96a7fdba8160f"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "98e59e83fa0a02779f4675248339d1f66e7868a3a946c9bf28591a8713c8517f"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "32c188cb9ad12d5941ed9d996cd4a5fe89571b25e5ac67e18806348372533c10"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "5f9e52c5240c75eecd69f3743d72a598c4350055cf7bf0945d02e787ae512d93"
    end
  end

  def install
    bin.install "omni"
  end

  def caveats
    <<~EOS
      Quick start:
        omni init          # Initialize OMNI setup
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
