use super::*;
use metra_lib::model::{Provider, ProviderSnapshot, ProviderStatus};

fn number(value: Option<u64>) -> String {
    value.map(|n| n.to_string()).unwrap_or_else(|| "—".into())
}
fn money(value: Option<u64>, currency: &str) -> String {
    value
        .map(|n| format!("{currency} {}.{:02}", n / 100, n % 100))
        .unwrap_or_else(|| "—".into())
}
fn status_key(status: ProviderStatus) -> &'static str {
    match status {
        ProviderStatus::Available => "status.available",
        ProviderStatus::DesktopInstalled => "status.desktopInstalled",
        ProviderStatus::NotInstalled => "status.notInstalled",
        ProviderStatus::NotLoggedIn => "status.notLoggedIn",
        ProviderStatus::Unsupported => "status.unsupported",
        ProviderStatus::NetworkError => "status.networkError",
        ProviderStatus::ProtocolError => "status.protocolError",
    }
}
fn metrics(snapshot: &ProviderSnapshot, t: I18n) -> Vec<(String, String)> {
    let mut values = Vec::new();
    if let Some(tokens) = &snapshot.tokens {
        let today = if snapshot.provider == Provider::Claude
            && snapshot
                .cost
                .as_ref()
                .is_some_and(|c| c.today_used_cents.is_some())
        {
            "provider.apiTodayTokensUtc"
        } else {
            "provider.localTodayTokens"
        };
        values.push((t.t(today), number(tokens.today)));
        values.push((
            t.t(if snapshot.provider == Provider::Codex {
                "provider.officialLifetimeTokens"
            } else {
                "provider.localLifetimeTokens"
            }),
            number(tokens.lifetime),
        ));
    }
    if let Some(cost) = &snapshot.cost {
        if cost.included_used_cents.is_some() || cost.included_limit_cents.is_some() {
            values.push((
                t.t("native.included"),
                format!(
                    "{} / {}",
                    money(cost.included_used_cents, &cost.currency),
                    money(cost.included_limit_cents, &cost.currency)
                ),
            ));
        }
        if cost.on_demand_enabled == Some(false) {
            values.push((t.t("native.ondemand"), t.t("common.disabled")));
        } else if cost.on_demand_used_cents.is_some() {
            values.push((
                t.t("native.ondemand"),
                format!(
                    "{} / {}",
                    money(cost.on_demand_used_cents, &cost.currency),
                    money(cost.on_demand_limit_cents, &cost.currency)
                ),
            ));
        }
        if cost.today_used_cents.is_some() {
            values.push((
                t.t("provider.apiEstimatedCostUtc"),
                money(cost.today_used_cents, &cost.currency),
            ));
        }
    }
    values
}
impl Dashboard {
    pub(super) fn usage_card(
        &self,
        provider: Provider,
        snapshot: &ProviderSnapshot,
        t: I18n,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = match provider {
            Provider::Cursor => "Cursor",
            Provider::Codex => "Codex",
            Provider::Claude => "Claude Code",
        };
        let running = self.refreshing.contains(&provider);
        let stale = snapshot.stale
            && (!snapshot.quotas.is_empty()
                || snapshot.tokens.is_some()
                || snapshot.cost.is_some());
        let mut card = div()
            .v_flex()
            .gap_2()
            .p_3()
            .border_1()
            .rounded_lg()
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .child(name)
                    .child(t.t(if running {
                        "refresh.refreshing"
                    } else if stale {
                        "status.stale"
                    } else {
                        status_key(snapshot.status)
                    })),
            )
            .child(snapshot.plan.clone().unwrap_or_else(|| t.t("plan.unknown")))
            .child(
                snapshot
                    .captured_at
                    .with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string(),
            );
        for quota in &snapshot.quotas {
            let remaining = quota.remaining_percent.clamp(0., 100.);
            let mut row = div()
                .v_flex()
                .gap_1()
                .child(
                    div()
                        .h_flex()
                        .justify_between()
                        .child(quota.label.clone())
                        .child(
                            t.format("quota.remaining", &[("value", format!("{remaining:.0}"))]),
                        ),
                )
                .child(
                    div()
                        .h(px(5.))
                        .w_full()
                        .bg(rgb(0x333a48))
                        .rounded_full()
                        .child(
                            div()
                                .h_full()
                                .w(relative(remaining as f32 / 100.))
                                .bg(rgb(0x9c83ff))
                                .rounded_full(),
                        ),
                )
                .child(t.format(
                    "quota.used",
                    &[("value", format!("{:.0}", quota.used_percent))],
                ));
            if let Some(reset) = quota.resets_at {
                let minutes = ((reset - chrono::Utc::now()).num_seconds().max(0) + 59) / 60;
                row = row.child(t.format(
                    "quota.resetsAt",
                    &[(
                        "date",
                        format!(
                            "{} · {}h {}m",
                            reset.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M"),
                            minutes / 60,
                            minutes % 60
                        ),
                    )],
                ));
            }
            card = card.child(row);
        }
        if snapshot.quotas.is_empty() {
            card = card.child(if running {
                t.t("provider.connectingDescription")
            } else {
                snapshot
                    .message
                    .as_deref()
                    .map(|m| t.message(m))
                    .unwrap_or_else(|| t.t("quota.noData"))
            });
        }
        for (label, value) in metrics(snapshot, t) {
            card = card.child(div().h_flex().justify_between().child(label).child(value));
        }
        if !snapshot.quotas.is_empty() {
            if let Some(message) = &snapshot.message {
                card = card.child(t.message(message));
            }
        }
        if provider == Provider::Cursor && !self.service.payload().settings.cursor_compat_enabled {
            card = card.child(
                Button::new("cursor-consent")
                    .label(t.t("cursor.readExactUsage"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Consent;
                        cx.notify();
                    })),
            );
        }
        card.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metra_lib::{
        model::{CostUsage, TokenUsage},
        settings::UiLanguage,
    };
    #[::core::prelude::v1::test]
    fn zero_costs_and_disabled_on_demand_are_preserved() {
        let mut data =
            ProviderSnapshot::unavailable(Provider::Claude, ProviderStatus::Available, "");
        data.tokens = Some(TokenUsage {
            today: Some(0),
            lifetime: Some(1000),
            peak_daily: None,
        });
        data.cost = Some(CostUsage {
            currency: "USD".into(),
            today_used_cents: Some(0),
            included_used_cents: Some(0),
            included_limit_cents: Some(2000),
            on_demand_used_cents: Some(0),
            on_demand_limit_cents: None,
            on_demand_enabled: Some(false),
            period_end: None,
        });
        let t = I18n::new(UiLanguage::En);
        let rows = metrics(&data, t);
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0], (t.t("provider.apiTodayTokensUtc"), "0".into()));
        assert_eq!(rows[2].1, "USD 0.00 / USD 20.00");
        assert_eq!(rows[3].1, t.t("common.disabled"));
        assert_eq!(rows[4].1, "USD 0.00");
    }
    #[::core::prelude::v1::test]
    fn missing_values_are_not_reported_as_zero() {
        assert_eq!(number(None), "—");
        assert_eq!(money(None, "USD"), "—");
    }
}
