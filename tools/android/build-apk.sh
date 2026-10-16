#!/bin/sh
# Build the Android app: tools/android/build-apk.sh [OUT.apk]
#
# Needs the Android SDK (ANDROID_HOME, default ~/Android/Sdk) with build-tools and a
# platform, the NDK (ANDROID_NDK_HOME, default the newest in $ANDROID_HOME/ndk), the
# Rust targets of ABIS, and a JDK. ABIS is a space-separated list of arm64-v8a
# (phones, the default) and x86_64 (the emulator). The game folder is LF2_DIR,
# default the repository's game/. The APK is signed with KEYSTORE (default
# ~/.android/debug.keystore, made when missing) and KEYSTORE_PASS (default android).
set -eu

repo=$(cd "$(dirname "$0")/../.." && pwd)
out=${1:-$repo/target/lf2-showdown.apk}
case $out in /*) ;; *) out=$PWD/$out ;; esac
sdk=${ANDROID_HOME:-$HOME/Android/Sdk}
ndk=${ANDROID_NDK_HOME:-$(ls -d "$sdk"/ndk/* | sort -V | tail -1)}
build_tools=$(ls -d "$sdk"/build-tools/* | sort -V | tail -1)
platform=$(ls -d "$sdk"/platforms/android-* | sort -V | tail -1)
abis=${ABIS:-arm64-v8a}
game=${LF2_DIR:-$repo/game}
keystore=${KEYSTORE:-$HOME/.android/debug.keystore}
pass=${KEYSTORE_PASS:-android}
min_sdk=26
toolchain=$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin
work=$repo/target/android

rm -rf "$work/apk" "$work/assets" "$work/classes"
mkdir -p "$work/apk" "$work/assets" "$work/classes"

# the native library, one per ABI
for abi in $abis; do
    case $abi in
        arm64-v8a) target=aarch64-linux-android ;;
        x86_64) target=x86_64-linux-android ;;
        *) echo "unknown ABI $abi (arm64-v8a or x86_64)"; exit 1 ;;
    esac
    t=$(echo "$target" | tr - _)
    T=$(echo "$t" | tr a-z A-Z)
    cc=$toolchain/$target$min_sdk-clang
    env "CARGO_TARGET_${T}_LINKER=$cc" "CC_$t=$cc" "CXX_$t=$cc++" "AR_$t=$toolchain/llvm-ar" \
        CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_STRIP=symbols LF2_DIR="$game" \
        cargo build --release --manifest-path "$repo/Cargo.toml" -p lf2-android --target "$target"
    mkdir -p "$work/apk/lib/$abi"
    cp "$repo/target/$target/release/liblf2_android.so" "$work/apk/lib/$abi/"
done

# the game folder as assets, with the list crates/lf2-android copies them by
cp -r "$game" "$work/assets/game"
(cd "$work/assets/game" && find . -type f ! -name files.txt | sed 's|^\./||' | LC_ALL=C sort |
    tr '\n' '\0' | xargs -0 sha256sum > files.txt)

# MainActivity
javac --release 11 -classpath "$platform/android.jar" -d "$work/classes" \
    $(find "$repo/android/java" -name '*.java')
"$build_tools/d8" --min-api $min_sdk --lib "$platform/android.jar" --output "$work/apk" \
    $(find "$work/classes" -name '*.class')

# resources, manifest and assets, then the code
"$build_tools/aapt2" compile --dir "$repo/android/res" -o "$work/res.zip"
"$build_tools/aapt2" link -o "$work/unaligned.apk" -I "$platform/android.jar" \
    --manifest "$repo/android/AndroidManifest.xml" -A "$work/assets" "$work/res.zip"
(cd "$work/apk" && zip -qr "$work/unaligned.apk" classes.dex lib)

if [ ! -f "$keystore" ]; then
    mkdir -p "$(dirname "$keystore")"
    keytool -genkeypair -keystore "$keystore" -storepass "$pass" -keypass "$pass" \
        -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 \
        -dname "CN=Android Debug,O=Android,C=US"
fi
mkdir -p "$(dirname "$out")"
"$build_tools/zipalign" -f -P 16 4 "$work/unaligned.apk" "$work/aligned.apk"
"$build_tools/apksigner" sign --ks "$keystore" --ks-pass "pass:$pass" --out "$out" "$work/aligned.apk"
echo "built $out"
