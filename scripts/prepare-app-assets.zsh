#!/bin/zsh
set -euo pipefail

root=${0:A:h:h}
source="$root/assets"
destination="$root/app/macos/resources"

for command in pngtopam pamcat pamchannel pamcut pamflip pamfunc pamscale pamstack pamtopng pgmmake pgmramp pnmarith ppmmake; do
  if ! command -v "$command" >/dev/null; then
    print -u2 "missing image tool: $command"
    exit 1
  fi
done

mkdir -p \
  "$destination/fonts" \
  "$destination/images/icons" \
  "$destination/images/groups" \
  "$destination/images/backgrounds" \
  "$destination/images/dialogs" \
  "$destination/images/settings" \
  "$destination/images/portraits" \
  "$destination/images/nine-patch/controls" \
  "$destination/images/nine-patch/dialogs" \
  "$destination/images/nine-patch/portraits" \
  "$destination/images/toast"

/usr/bin/install -m 644 "$source/png/ui/toast-background.png" \
  "$destination/images/toast/toast-background.png"
/usr/bin/install -m 644 "$source/png/ui/toast-badge.png" \
  "$destination/images/toast/toast-badge.png"

/usr/bin/install -m 644 "$source/fonts/app"/*.{ttf,otf}(N) "$destination/fonts/"
/usr/bin/install -m 644 "$source/fonts/game"/*.{ttf,otf}(N) "$destination/fonts/"
/usr/bin/install -m 644 "$source/png/portraits"/*.png "$destination/images/portraits/"
/usr/bin/install -m 644 "$source/png/portraits/marine.png" \
  "$destination/images/icons/account-placeholder.png"
/usr/bin/install -m 644 "$source/png/groups/clan.png" \
  "$destination/images/groups/clan.png"
/usr/bin/install -m 644 "$source/png/groups/community.png" \
  "$destination/images/groups/community.png"
/usr/bin/install -m 644 "$source/png/groups/barcraft.png" \
  "$destination/images/groups/barcraft.png"
/usr/bin/install -m 644 "$source/png/groups/esports-teams.png" \
  "$destination/images/groups/esports-teams.png"
/usr/bin/install -m 644 "$source/png/groups/coaching.png" \
  "$destination/images/groups/coaching.png"
/usr/bin/install -m 644 "$source/png/groups/company.png" \
  "$destination/images/groups/company.png"
/usr/bin/install -m 644 "$source/png/groups/region.png" \
  "$destination/images/groups/region.png"
/usr/bin/install -m 644 "$source/png/groups/school.png" \
  "$destination/images/groups/school.png"
/usr/bin/install -m 644 "$source/png/groups/shoutcast.png" \
  "$destination/images/groups/shoutcast.png"
/usr/bin/install -m 644 "$source/png/groups/other.png" \
  "$destination/images/groups/other.png"
/usr/bin/install -m 644 "$source/png/groups/esports-leagues.png" \
  "$destination/images/groups/esports-leagues.png"
/usr/bin/install -m 644 "$source/png/groups/arcade.png" \
  "$destination/images/groups/arcade.png"
/usr/bin/install -m 644 "$source/png/groups/igr.png" \
  "$destination/images/groups/igr.png"
/usr/bin/install -m 644 "$source/png/icons/chat-flat.png" \
  "$destination/images/icons/chat.png"
/usr/bin/install -m 644 "$source/png/icons/chat-battlenet.png" \
  "$destination/images/icons/battle-net.png"
/usr/bin/install -m 644 "$source/png/icons/chat-broadcast.png" \
  "$destination/images/icons/broadcast.png"
/usr/bin/install -m 644 "$source/png/icons/chat-channel.png" \
  "$destination/images/icons/channel.png"
/usr/bin/install -m 644 "$source/png/icons/friends.png" \
  "$destination/images/icons/friends.png"
/usr/bin/install -m 644 "$source/png/icons/status-available.png" \
  "$destination/images/icons/status-available.png"
/usr/bin/install -m 644 "$source/png/icons/status-away.png" \
  "$destination/images/icons/status-away.png"
/usr/bin/install -m 644 "$source/png/icons/status-busy.png" \
  "$destination/images/icons/status-busy.png"
/usr/bin/install -m 644 "$source/png/icons/status-offline.png" \
  "$destination/images/icons/status-offline.png"
/usr/bin/install -m 644 "$source/png/icons/status-in-game.png" \
  "$destination/images/icons/status-in-game.png"
/usr/bin/install -m 644 "$source/png/ui/portrait-frame.png" \
  "$destination/images/nine-patch/portraits/frame.png"
/usr/bin/install -m 644 "$source/png/ui/settings-checkbox-mark.png" \
  "$destination/images/settings/checkbox-mark.png"
/usr/bin/install -m 644 "$source/png/ui/settings-tooltip-fill.png" \
  "$destination/images/settings/tooltip-fill.png"
/usr/bin/install -m 644 "$source/png/ui/settings-tooltip-outline.png" \
  "$destination/images/settings/tooltip-outline.png"
# the top-nav textures are intentionally NOT installed from $source/png/ui:
# their retail DDS sources are additive-blend art with an all-zero alpha
# channel, so the straight sips conversion in
# research/extract-sc2-assets.zsh produces
# fully transparent PNGs. the app loads hand-recovered copies from
# $destination/images/curated/controls instead.
for layer in selected selected-line selected-line-glow; do
  base="$destination/images/curated/controls/top-nav-$layer.png"
  pamstack -tupletype=RGB_ALPHA \
    <(pngtopam -alphapam "$base" | pamchannel 2) \
    <(pngtopam -alphapam "$base" | pamchannel 1) \
    <(pngtopam -alphapam "$base" | pamchannel 0) \
    <(pngtopam -alphapam "$base" | pamchannel 3) \
    | pamtopng > "$destination/images/curated/controls/top-nav-$layer-orange.png"
  pamstack -tupletype=RGB_ALPHA \
    <(pngtopam -alphapam "$base" | pamchannel 2 | pamfunc -multiplier 0.82) \
    <(pngtopam -alphapam "$base" | pamchannel 0) \
    <(pamarith -mean \
      <(pngtopam -alphapam "$base" | pamchannel 2) \
      <(pngtopam -alphapam "$base" | pamchannel 0) \
      | pamfunc -multiplier 0.8) \
    <(pngtopam -alphapam "$base" | pamchannel 3) \
    | pamtopng > "$destination/images/curated/controls/top-nav-$layer-pink.png"
done
modal_dialog="$source/ui-chrome/png/mods/core.sc2mod/base.sc2assets/assets/textures/ui_battlenet_glues_pageassets_dialogstandardbg.png"
pamcat -topbottom \
  <(pngtopam -alphapam "$modal_dialog" | pamcut -left 0 -top 0 -width 452 -height 22) \
  <(pamcat -leftright \
    <(pngtopam -alphapam "$modal_dialog" | pamcut -left 0 -top 22 -width 22 -height 88) \
    <(pamstack -tupletype=RGB_ALPHA <(ppmmake black 408 88) <(pgmmake 0 408 88)) \
    <(pngtopam -alphapam "$modal_dialog" | pamcut -left 430 -top 22 -width 22 -height 88)) \
  <(pngtopam -alphapam "$modal_dialog" | pamcut -left 0 -top 110 -width 452 -height 22) \
  | pamtopng > "$destination/images/nine-patch/dialogs/modal-frame.png"
/usr/bin/install -m 644 \
  "$source/ui-chrome/png/mods/core.sc2mod/base.sc2assets/assets/textures/ui_glues_pageassets_loadingtitlebg.png" \
  "$destination/images/dialogs/modal-title-band.png"
# the honeycomb wash that sits behind every retail dialog. the texture is a
# flat #0084FF with the pattern carried entirely in its alpha channel, so it
# installs straight across with no channel surgery.
/usr/bin/install -m 644 "$source/png/ui/modal-hex-pattern.png" \
  "$destination/images/dialogs/modal-hex.png"
# the scanline halo that bleeds outward from the dialog's side rails onto
# whatever is behind it. the texture is brightest down its right edge, and each
# copy sits outside the card with that bright edge turned inward, so the
# unflipped one goes on the left and the mirrored one on the right.
# the same texture serves all four rails: rotating it turns the bright edge to
# whichever side faces the card.
modal_glow="$source/png/ui/modal-glow.png"
pngtopam -alphapam "$modal_glow" \
  | pamtopng > "$destination/images/dialogs/modal-glow-left.png"
pngtopam -alphapam "$modal_glow" \
  | pamflip -lr \
  | pamtopng > "$destination/images/dialogs/modal-glow-right.png"
pngtopam -alphapam "$modal_glow" \
  | pamflip -rotate270 \
  | pamtopng > "$destination/images/dialogs/modal-glow-top.png"
pngtopam -alphapam "$modal_glow" \
  | pamflip -rotate90 \
  | pamtopng > "$destination/images/dialogs/modal-glow-bottom.png"

warning_dialog="$source/ui-chrome/png/mods/core.sc2mod/base.sc2assets/assets/textures/ui_battlenet_glues_pageassets_dialogbg_error.png"
warning_glow="$source/ui-chrome/png/mods/core.sc2mod/base.sc2assets/assets/textures/ui_battlenet_glues_pageassets_dialogbg_gloworange.png"
warning_hex="$source/ui-chrome/png/mods/core.sc2mod/base.sc2assets/assets/textures/ui_battlenet_glues_pageassets_dialog_hexpatternorange.png"
warning_button_idle="$source/ui-chrome/png/mods/core.sc2mod/base.sc2assets/assets/textures/ui_battlenet_glue_navbuttons_orange_normalpressed.png"
warning_button_active="$source/ui-chrome/png/mods/core.sc2mod/base.sc2assets/assets/textures/ui_battlenet_glue_navbuttons_orange_normaloverpressedover.png"
button_idle="$source/ui-chrome/png/mods/core.sc2mod/base.sc2assets/assets/textures/ui_battlenet_glue_navbuttons_blue_normalpressed.png"
button_active="$source/ui-chrome/png/mods/core.sc2mod/base.sc2assets/assets/textures/ui_battlenet_glue_navbuttons_blue_normaloverpressedover.png"

pamcat -topbottom \
  <(pamcat -leftright \
    <(pngtopam -alphapam "$warning_dialog" | pamcut -left 0 -top 0 -width 184 -height 32) \
    <(pngtopam -alphapam "$warning_dialog" | pamcut -left 183 -top 0 -width 2 -height 32 | pamscale -xsize 252 -ysize 32) \
    <(pngtopam -alphapam "$warning_dialog" | pamcut -left 184 -top 0 -width 184 -height 32)) \
  <(pamcat -leftright \
    <(pngtopam -alphapam "$warning_dialog" | pamcut -left 0 -top 88 -width 32 -height 2 | pamscale -xsize 32 -ysize 246) \
    <(pamstack -tupletype=RGB_ALPHA <(ppmmake black 556 246) <(pgmmake 0 556 246)) \
    <(pngtopam -alphapam "$warning_dialog" | pamcut -left 336 -top 88 -width 32 -height 2 | pamscale -xsize 32 -ysize 246)) \
  <(pamcat -leftright \
    <(pngtopam -alphapam "$warning_dialog" | pamcut -left 368 -top 152 -width 184 -height 32) \
    <(pngtopam -alphapam "$warning_dialog" | pamcut -left 551 -top 152 -width 2 -height 32 | pamscale -xsize 252 -ysize 32) \
    <(pngtopam -alphapam "$warning_dialog" | pamcut -left 552 -top 152 -width 184 -height 32)) \
  | pamtopng > "$destination/images/dialogs/warning-frame.png"
pngtopam -alphapam "$warning_glow" \
  | pamtopng > "$destination/images/dialogs/warning-glow-left.png"
pngtopam -alphapam "$warning_glow" \
  | pamflip -lr \
  | pamtopng > "$destination/images/dialogs/warning-glow-right.png"
pamstack -tupletype=RGB_ALPHA \
  <(ppmmake '#ff8a22' 700 180) \
  <(pnmarith -multiply \
    <(pngtopam "$warning_hex" \
      | pamcut -left 0 -top 0 -width 700 -height 180 \
      | pamchannel 1 \
      | pamfunc -subtractor=125 \
      | pamfunc -multiplier=30) \
    <(pnmarith -multiply \
      <(pgmramp -tb 700 180 | pamflip -tb) \
      <(pgmramp -tb 700 180 | pamflip -tb))) \
  | pamtopng > "$destination/images/dialogs/warning-hex-top.png"
pngtopam -alphapam "$destination/images/dialogs/warning-hex-top.png" \
  | pamflip -tb \
  | pamtopng > "$destination/images/dialogs/warning-hex-bottom.png"
pngtopam -alphapam "$warning_button_idle" \
  | pamcut -top 0 -height 76 \
  | pamtopng > "$destination/images/nine-patch/controls/warning-button-idle.png"
pngtopam -alphapam "$warning_button_active" \
  | pamcut -top 0 -height 76 \
  | pamtopng > "$destination/images/nine-patch/controls/warning-button-active.png"

typeset -A chat_backgrounds=(
  deep-nebula 'campaigns/void.sc2campaign/base.sc2assets/assets/textures/background_nebula.png'
  shakuras-nebula 'campaigns/void.sc2campaign/base.sc2assets/assets/textures/smx2_background_shakuras_nebula_cloud_dif.png'
  aiur-dusk 'campaigns/void.sc2campaign/base.sc2assets/assets/textures/smx2_aiur1_debrief_skybox.png'
  swarm-horizon 'mods/core.sc2mod/base.sc2assets/assets/textures/mainmenuswarmbg_low.png'
  frozen-moon 'campaigns/swarm.sc2campaign/base.sc2assets/assets/textures/smex1_iceworldsetbg.png'
  midnight-front 'campaigns/swarm.sc2campaign/base.sc2assets/assets/textures/smx1_mobiusset_skybg_night.png'
  solar-fury 'campaigns/liberty.sc2campaign/base.sc2assets/assets/textures/sm_hb_bg.png'
  last-stand 'mods/core.sc2mod/base.sc2assets/assets/textures/ui_glues_newuser_bg.png'
)

background_staging=$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/superiority-backgrounds.XXXXXX")
trap '/usr/bin/find "$background_staging" -type f -delete; /bin/rmdir "$background_staging"' EXIT
for name relative in ${(kv)chat_backgrounds}; do
  input="$source/chat-backgrounds/png/$relative"
  width=$(/usr/bin/sips -g pixelWidth "$input" | /usr/bin/awk '/pixelWidth:/ { print $2 }')
  height=$(/usr/bin/sips -g pixelHeight "$input" | /usr/bin/awk '/pixelHeight:/ { print $2 }')
  if (( width * 3 > height * 4 )); then
    crop_height=$height
    crop_width=$((height * 4 / 3))
  else
    crop_width=$width
    crop_height=$((width * 3 / 4))
  fi
  offset_x=$(((width - crop_width) / 2))
  offset_y=$(((height - crop_height) / 2))
  cropped="$background_staging/$name-cropped.png"
  /usr/bin/sips --cropToHeightWidth "$crop_height" "$crop_width" \
    --cropOffset "$offset_y" "$offset_x" "$input" --out "$cropped" >/dev/null
  /usr/bin/sips --resampleHeightWidth 1200 1600 "$cropped" \
    --out "$destination/images/backgrounds/$name.png" >/dev/null
done

pngtopam "$source/png/ui/tab-hover.png" \
  | pamcut -top 0 -height 68 \
  | pamtopng > "$destination/images/nine-patch/controls/segmented-tab-idle.png"
pngtopam "$source/png/ui/tab-selected.png" \
  | pamcut -top 0 -height 68 \
  | pamtopng > "$destination/images/nine-patch/controls/segmented-tab-active.png"
pngtopam -alphapam "$button_idle" \
  | pamcut -top 0 -height 76 \
  | pamtopng > "$destination/images/nine-patch/controls/button-idle.png"
pngtopam -alphapam "$button_active" \
  | pamcut -top 0 -height 76 \
  | pamtopng > "$destination/images/nine-patch/controls/button-active.png"

print "$destination"
