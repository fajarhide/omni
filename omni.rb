class Omni < Formula
  desc "Semantic Signal Engine — Less noise. More signal. Right signal."
  homepage "https://github.com/fajarhide/omni"
  version "0.5.4-rc2"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "3dfa68198b7af5ada774a97b02dd2853f4b029ad16b0791d412c0e426c16228b"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "0744d3463d48e44130e3aca05cb2015e9977aef21083dd5bdbf0433ea97cb562"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "13ff137e4a590925438b86e568f5b63adb2f9225cd328df83072601a6430e7ca"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "fabc660e69347a4d5bb7b43ef5b504a5cbd2a28d25ce90695ef58f5fa2011a3e"
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
