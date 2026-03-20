class Omni < Formula
  desc "Semantic Distillation Engine for the Agentic AI"
  homepage "https://github.com/fajarhide/omni"
  url "https://github.com/fajarhide/omni/archive/refs/tags/v0.4.5.tar.gz"
  sha256 "235a521ec85d033eab1be5b0f756f8406a8c718e720f25b908ae1e1191316648"
  license "MIT"

  depends_on "zig" => :build
  depends_on "node"

  def install
    # Run builds from the 'core' directory
    Dir.chdir("core") do
      # Native binary -> bin/omni
      system "zig", "build", "-Doptimize=ReleaseFast", "-Dversion=#{version}", "-p", "../"
      # Wasm binary -> bin/omni-wasm.wasm
      system "zig", "build", "wasm", "-Doptimize=ReleaseSmall", "-Dversion=#{version}", "-p", "../"
    end

    # Install Native Binary
    bin.install "bin/omni"

    # Install MCP Server to libexec
    libexec.install "package.json", "package-lock.json", "tsconfig.json", "src"
    cd libexec do
      system "npm", "install"
      system "./node_modules/.bin/tsc"
      system "npm", "prune", "--omit=dev"
    end

    # Install Wasm Binary alongside MCP Server so __dirname paths work correctly
    (libexec/"core").install "bin/omni-wasm.wasm"
  end

  def caveats
    <<~EOS
      OMNI SETUP & INTEGRATION GUIDE
      ══════════════════════════════════════════════════════════

      To complete the setup and configure the MCP server, run:
        omni setup
    EOS
  end

  test do
    assert_match "omni", shell_output("#{bin}/omni --help")
  end
end
