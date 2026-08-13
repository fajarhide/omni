class Omni < Formula
  desc "Semantic Signal Engine: less noise, more signal, the right signal"
  homepage "https://github.com/fajarhide/omni"
  version "0.7.4"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "7a661b9d82e27eaae89b9a55bddaafaffc48664851b68ffd95d703a961a3ccae"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "5c8ad3043c70678d3e4a59b164c976fb8fd5c1801344173ce4699b37a37b5601"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "dabfaeb0b9fb701ec474ef489f929f332a82be051f8d4ddd80c6c6dc36032f25"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "0e4da95ff44f502c4f8c765d1ce6af59c1d3c87c5f8d4778dcca5a40e53881b1"
    end
  end

  def install
    bin.install "omni"
  end

  def caveats
    <<~EOS
      Quick start:
        omni init              # Initialize OMNI setup (interactive)
        omni doctor            # Verify installation
        omni stats             # View token savings

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
