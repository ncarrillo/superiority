#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
core_stage="$root/stimpak/csharp/artifacts/runtimes"
auth_stage="$root/stimpak/csharp/Stimpak.Auth/artifacts/runtimes"
core_project="$root/stimpak/csharp/Stimpak/Stimpak.csproj"
auth_project="$root/stimpak/csharp/Stimpak.Auth/Stimpak.Auth.csproj"
output="$root/dist/nuget"
package_version=${STIMPAK_PACKAGE_VERSION:-}
version_args=()

if [[ -n "$package_version" ]]; then
  version_args=(
    -p:Version="$package_version"
    -p:PackageVersion="$package_version"
  )
fi

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

dotnet run --project "$root/stimpak/csharp/Stimpak.Tests/Stimpak.Tests.csproj" -c Release
/usr/bin/install -d "$output"
dotnet pack "$core_project" \
  -c Release \
  -p:StimpakBuildNative=false \
  $version_args \
  -o "$output"
dotnet pack "$auth_project" \
  -c Release \
  -p:StimpakBuildNative=false \
  -p:StimpakAuthBuildNative=false \
  $version_args \
  -o "$output"

print "Stimpak NuGet packages: $output"
