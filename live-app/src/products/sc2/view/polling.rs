use super::*;

impl LiveView {
    pub(super) fn refresh_channel(
        &mut self,
        channel: String,
        include_roster: bool,
        cx: &mut Context<Self>,
    ) {
        let now = js_sys::Date::now();
        let data = self.channel_data.entry(channel.clone()).or_default();
        if data.loading
            || data
                .refreshed_at
                .is_some_and(|refreshed_at| now - refreshed_at < CHANNEL_REFRESH_MS)
        {
            return;
        }
        let message_cursor = data.message_cursor;
        data.loading = true;
        let backend = self.backend.clone();
        let feed_id = self.feed_id.clone();
        cx.spawn(async move |this, cx| {
            let stream = fetch_stream(&backend, &feed_id, &channel, message_cursor).await;
            let roster = if include_roster {
                Some(fetch_roster(&backend, &feed_id, &channel).await)
            } else {
                None
            };
            let _ = this.update(cx, |view, cx| {
                let selected = view.selected.as_deref() == Some(channel.as_str());
                let stream_error = stream.as_ref().err().cloned();
                let roster_error = roster
                    .as_ref()
                    .and_then(|result| result.as_ref().err().cloned());
                let mut ready = false;
                let mut stream_ready = false;
                {
                    let data = view.channel_data.entry(channel.clone()).or_default();
                    data.loading = false;
                    if let Ok((mut items, next_message)) = stream {
                        items.retain(|candidate| {
                            !data
                                .items
                                .iter()
                                .any(|existing| existing.stable_key() == candidate.stable_key())
                        });
                        data.items.append(&mut items);
                        data.items.sort_by_key(StreamItem::sort_key);
                        if data.items.len() > 500 {
                            data.items.drain(..data.items.len() - 500);
                        }
                        data.message_cursor = Some(next_message);
                        data.stream_loaded = true;
                        stream_ready = true;
                        ready = !include_roster;
                    }
                    if let Some(Ok(roster)) = roster {
                        data.roster = roster;
                        ready = stream_ready;
                    }
                    if ready {
                        data.refreshed_at = Some(js_sys::Date::now());
                    }
                }
                if ready {
                    view.start_pending_transition(&channel);
                }
                if ready && selected {
                    view.workspace.transcript.scroll.scroll_to_bottom();
                } else if selected {
                    view.error = stream_error.or(roster_error);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn start_polling(&self, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let feed_id = self.feed_id.clone();
        cx.spawn(async move |this, cx| {
            let mut tick = 0_u64;
            let mut loaded_channel: Option<String> = None;

            loop {
                match this.read_with(cx, |view, _| view.active) {
                    Ok(true) => {}
                    Ok(false) => {
                        gloo_timers::future::TimeoutFuture::new(500).await;
                        continue;
                    }
                    Err(_) => break,
                }
                let mut snapshot_changed = false;
                if tick.is_multiple_of(4) {
                    match fetch_overview(&backend, &feed_id).await {
                        Ok((status, mut channels)) => {
                            if status.product != FeedProduct::Sc2 {
                                let product = status.product;
                                let _ = this.update(cx, |view, cx| {
                                    view.error = Some(format!(
                                        "This SC2 viewer cannot display a {} feed.",
                                        product.label()
                                    ));
                                    cx.notify();
                                });
                                break;
                            }
                            let session_id =
                                status.session.as_ref().map(|session| session.id.clone());
                            snapshot_changed = this
                                .read_with(cx, |view, _| view.session_id != session_id)
                                .unwrap_or(false);
                            channels.sort_by(|left, right| {
                                left.first_seen_at
                                    .cmp(&right.first_seen_at)
                                    .then_with(|| left.key.cmp(&right.key))
                            });
                            if this
                                .update(cx, |view, cx| {
                                    if view.session_id != session_id && session_id.is_some() {
                                        view.session_id = session_id.clone();
                                        view.channel_transition = None;
                                        view.workspace.roster.selections.clear();
                                    }
                                    view.feed_state = status.state.clone();
                                    view.had_session = status.session.is_some();
                                    reconcile_channel_order(&mut view.channels, channels);
                                    view.workspace.roster.selections.retain(|key, _| {
                                        view.channels.iter().any(|channel| &channel.key == key)
                                    });
                                    view.roster_filters.retain(|key, _| {
                                        view.channels.iter().any(|channel| &channel.key == key)
                                    });
                                    if view.selected.as_ref().is_none_or(|selected| {
                                        !view
                                            .channels
                                            .iter()
                                            .any(|channel| &channel.key == selected)
                                    }) {
                                        view.selected = view
                                            .channels
                                            .first()
                                            .map(|channel| channel.key.clone());
                                    }
                                    view.error = None;
                                    let preload = view
                                        .channels
                                        .iter()
                                        .filter(|channel| {
                                            !view
                                                .channel_data
                                                .get(&channel.key)
                                                .is_some_and(|data| data.stream_loaded)
                                        })
                                        .map(|channel| channel.key.clone())
                                        .collect::<Vec<_>>();
                                    for channel in preload {
                                        view.refresh_channel(channel, true, cx);
                                    }
                                    cx.notify();
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(error) => {
                            if this
                                .update(cx, |view, cx| {
                                    view.error = Some(error);
                                    cx.notify();
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                }

                let selected = match this.read_with(cx, |view, _| view.selected.clone()) {
                    Ok(selected) => selected,
                    Err(_) => break,
                };
                let channel_changed = selected != loaded_channel;
                if channel_changed {
                    loaded_channel = selected.clone();
                }

                if let Some(channel) = selected.as_deref() {
                    let (message_cursor, loading, transition_pending) = this
                        .read_with(cx, |view, _| {
                            let transition_pending =
                                view.channel_transition.as_ref().is_some_and(|transition| {
                                    transition.incoming == channel && transition.is_pending()
                                });
                            view.channel_data
                                .get(channel)
                                .map_or((None, false, transition_pending), |data| {
                                    (data.message_cursor, data.loading, transition_pending)
                                })
                        })
                        .unwrap_or((None, false, false));
                    let refresh_selected =
                        channel_changed || transition_pending || snapshot_changed;
                    let mut transition_stream_ready = !transition_pending;

                    if !loading && (refresh_selected || tick.is_multiple_of(6)) {
                        let message_cursor = if transition_pending {
                            None
                        } else {
                            message_cursor
                        };
                        match fetch_stream(&backend, &feed_id, channel, message_cursor).await {
                            Ok((mut fresh, next_message)) => {
                                let expected = channel.to_owned();
                                if this
                                    .update(cx, |view, cx| {
                                        let selected =
                                            view.selected.as_deref() == Some(expected.as_str());
                                        let data =
                                            view.channel_data.entry(expected.clone()).or_default();
                                        if transition_pending {
                                            data.items = fresh;
                                        } else {
                                            fresh.retain(|candidate| {
                                                !data.items.iter().any(|existing| {
                                                    existing.stable_key() == candidate.stable_key()
                                                })
                                            });
                                            data.items.append(&mut fresh);
                                        }
                                        data.items.sort_by_key(StreamItem::sort_key);
                                        if data.items.len() > 500 {
                                            data.items.drain(..data.items.len() - 500);
                                        }
                                        data.message_cursor = Some(next_message);
                                        data.stream_loaded = true;
                                        data.refreshed_at = Some(js_sys::Date::now());
                                        view.error = None;
                                        if selected {
                                            view.workspace.transcript.scroll.scroll_to_bottom();
                                        }
                                        cx.notify();
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                                transition_stream_ready = true;
                            }
                            Err(error) => {
                                let _ = this.update(cx, |view, cx| {
                                    view.error = Some(error);
                                    cx.notify();
                                });
                            }
                        }
                    }

                    if !loading && (refresh_selected || tick.is_multiple_of(30)) {
                        match fetch_roster(&backend, &feed_id, channel).await {
                            Ok(roster) => {
                                let expected = channel.to_owned();
                                if this
                                    .update(cx, |view, cx| {
                                        let data =
                                            view.channel_data.entry(expected.clone()).or_default();
                                        data.roster = roster;
                                        data.refreshed_at = Some(js_sys::Date::now());
                                        if transition_pending && transition_stream_ready {
                                            view.start_pending_transition(&expected);
                                        }
                                        cx.notify();
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(error) => {
                                let _ = this.update(cx, |view, cx| {
                                    view.error = Some(error);
                                    cx.notify();
                                });
                            }
                        }
                    }
                }

                if tick.is_multiple_of(2)
                    && this
                        .update(cx, |view, cx| {
                            let inactive = view
                                .channels
                                .iter()
                                .filter(|channel| {
                                    view.selected.as_deref() != Some(channel.key.as_str())
                                })
                                .map(|channel| channel.key.clone())
                                .collect::<Vec<_>>();
                            if !inactive.is_empty() {
                                let index = (tick as usize / 2) % inactive.len();
                                view.refresh_channel(inactive[index].clone(), true, cx);
                            }
                        })
                        .is_err()
                {
                    break;
                }

                tick = tick.wrapping_add(1);
                gloo_timers::future::TimeoutFuture::new(500).await;
            }
        })
        .detach();
    }
}
