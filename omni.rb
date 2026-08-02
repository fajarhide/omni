class Omni < Formula
  desc "Semantic Signal Engine — Less noise. More signal. Right signal."
  homepage "https://github.com/fajarhide/omni"
  version "0.6.9"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "d23debbe469358465332f2c296fea2e8c5ea38aca786fd0d82d7f8c0ccb120a8"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "969e3a2a50de5a38ed47492a76e1bed803c0e95614230b7e91f62ca2d43f395a"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "3f2f2b2876d96375b7ee0b07643b613f6d7f8f9574f74ed589cdd1847c6ad6ef"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "2a17a1cb393074c67f1af0466fca3534327f15cc33e6027282e60798dd039092"
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
