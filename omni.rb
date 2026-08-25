class Omni < Formula
  desc "Semantic Signal Engine: less noise, more signal, the right signal"
  homepage "https://github.com/fajarhide/omni"
  version "0.7.8"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "89c75110ccb8ac3aae5893e090fa50e8895d3ff5fbba0c7cec9aa5003a6993ab"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "bb5f4cba8d56592edc0c4f5857c5eb5fe4818f54ceea1147b2d71c255c40829d"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "2c49379e3286f44912fa43df95e62ee805a2b4670c772ab1219c6b403ea7de19"
    end
    on_intel do
      url "https://github.com/fajarhide/omni/releases/download/v#{version}/omni-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "f20bdf79e9d57e6bf31084b0f9cd1753fa2114fe5f25a6c02a41daad4e523c7c"
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
