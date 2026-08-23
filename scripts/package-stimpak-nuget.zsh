#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
stage="$root/stimpak/csharp/artifacts/runtimes"
project="$root/stimpak/csharp/Stimpak/Stimpak.csproj"
output="$root/dist/nuget"

if [[ $(uname -s) != Darwin ]]; then
  print -u2 "Stimpak release packaging runs on macOS"
  exit 1
fi
if [[ ! -d "$stage" ]] || [[ -z $(/usr/bin/find "$stage" -type f -print -quit) ]]; then
  print -u2 "no staged Stimpak runtimes under $stage"
  exit 1
fi

windows=(${stage}/win-*/native/*.(dll|exe)(N))
if (( ${#windows} )); then
  if ! command -v osslsigncode >/dev/null; then
    print -u2 "osslsigncode is required to verify staged Windows signatures"
    exit 1
  fi
  for artifact in $windows; do
    if ! osslsigncode verify -in "$artifact" >/dev/null; then
      print -u2 "refusing to package unsigned Windows artifact: $artifact"
      exit 1
    fi
  done
fi

dotnet run --project "$root/stimpak/csharp/Stimpak.Tests/Stimpak.Tests.csproj" -c Release
/usr/bin/install -d "$output"
dotnet pack "$project" \
  -c Release \
  -p:StimpakBuildNative=false \
  -o "$output"

print "Stimpak NuGet package: $output"
