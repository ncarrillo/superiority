#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
core_stage="$root/stimpak/csharp/artifacts/runtimes"
auth_stage="$root/stimpak/csharp/Stimpak.Auth/artifacts/runtimes"
core_project="$root/stimpak/csharp/Stimpak/Stimpak.csproj"
auth_project="$root/stimpak/csharp/Stimpak.Auth/Stimpak.Auth.csproj"
output="$root/dist/nuget"

if [[ $(uname -s) != Darwin ]]; then
  print -u2 "Stimpak release packaging runs on macOS"
  exit 1
fi
for stage in "$core_stage" "$auth_stage"; do
  if [[ ! -d "$stage" ]] || [[ -z $(/usr/bin/find "$stage" -type f -print -quit) ]]; then
    print -u2 "no staged Stimpak runtimes under $stage"
    exit 1
  fi
done

windows=(${core_stage}/win-*/native/*.dll(N) ${auth_stage}/win-*/native/*.dll(N))
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
dotnet pack "$core_project" \
  -c Release \
  -p:StimpakBuildNative=false \
  -o "$output"
dotnet pack "$auth_project" \
  -c Release \
  -p:StimpakBuildNative=false \
  -p:StimpakAuthBuildNative=false \
  -o "$output"

print "Stimpak NuGet packages: $output"
