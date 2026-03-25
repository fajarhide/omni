class Omni < Formula
  desc "Semantic Signal Engine — Less noise. More signal. Right signal."
  homepage "https://github.com/fajarhide/omni"
  version "0.5.4-rc3"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "3d1650c15289b47ab829bcd0007cea20551a0690928bfa53c83ddffc178c4c11"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "21f99376fa5db62e80514adadf300524a988f9396e05615c2d97038ac7207ec8"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "9bcc15a9070faba2dc66120a1a44cb01ebcb6b9b0024c5948fa38b058584c52e"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "a4cc92ad035bbb25cc04e544bb63ef53ace2c4c601b3509cafcb08ff19e39a8f"
    end
  end

  def install
    bin.install "omni"
  end

  def caveats
    <<~EOS
      Quick start:
        omni init --all        # Initialize OMNI setup
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
