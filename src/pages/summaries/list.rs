use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::features::{
    auth::models::UserSession,
    projects::handlers::get_project,
    summaries::{delete_summary, list_summaries_for_project, SummaryListItem},
};

#[component]
pub fn SummariesPage() -> impl IntoView {
    let user_resource =
        expect_context::<LocalResource<Result<Option<UserSession>, ServerFnError>>>();
    let user = Signal::derive(move || user_resource.get().and_then(|r| r.ok()).flatten());

    let params = use_params_map();
    let project_id = move || params.with(|p| p.get("id").and_then(|id| id.parse::<i64>().ok()));

    let project_resource = LocalResource::new(move || {
        let id = project_id();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid project"))?;
            get_project(id).await
        }
    });

    let summaries_resource = LocalResource::new(move || {
        let id = project_id();
        async move {
            let id = id.ok_or_else(|| ServerFnError::new("Invalid project"))?;
            list_summaries_for_project(id).await
        }
    });

    let delete_action = Action::new(|summary_id: &i64| {
        let summary_id = *summary_id;
        async move { delete_summary(summary_id).await }
    });

    Effect::new(move |_| {
        if delete_action.value().get().is_some() {
            summaries_resource.refetch();
        }
    });

    Effect::new(move |_| {
        if let Some(Ok(summaries)) = summaries_resource.get() {
            let has_generating = summaries
                .iter()
                .any(|s| s.status == "pending" || s.status == "processing");
            if has_generating {
                set_timeout(
                    move || {
                        summaries_resource.refetch();
                    },
                    std::time::Duration::from_secs(2),
                );
            }
        }
    });

    view! {
        <section class="mx-auto flex min-h-screen max-w-5xl flex-col gap-8 px-6 py-16">
            <Show
                when=move || user.get().is_some()
                fallback=move || view! {
                    <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-6 text-slate-300">
                        "Please log in to view summaries."
                    </div>
                }
            >
                <Show
                    when=move || project_resource.get().is_some()
                    fallback=move || view! { <p class="text-sm text-slate-400">"Loading project..."</p> }
                >
                    {move || match project_resource.get() {
                        Some(Ok(project)) => view! {
                            <div class="space-y-2">
                                <a
                                    class="text-sm text-slate-400 hover:text-white"
                                    href=format!("/projects/{}", project.id)
                                >"← Back to project"</a>
                                <h1 class="text-4xl font-semibold text-white">"Study Summaries"</h1>
                                <p class="text-slate-300">{format!("Project: {}", project.name)}</p>
                            </div>

                            <Show
                                when=move || summaries_resource.get().is_some()
                                fallback=move || view! { <p class="text-sm text-slate-400">"Loading summaries..."</p> }
                            >
                                {move || match summaries_resource.get() {
                                    Some(Ok(summaries)) if summaries.is_empty() => view! {
                                        <div class="rounded-2xl border border-slate-800 bg-slate-900/40 p-12 text-center">
                                            <p class="text-slate-400">"No summaries yet. Generate one from a PDF file!"</p>
                                        </div>
                                    }.into_any(),
                                    Some(Ok(summaries)) => view! {
                                        <div class="grid gap-4 md:grid-cols-2">
                                            {summaries.into_iter().map(|summary| {
                                                view! {
                                                    <SummaryCard summary=summary delete_action=delete_action />
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any(),
                                    Some(Err(err)) => view! {
                                        <p class="text-sm text-rose-300">{err.to_string()}</p>
                                    }.into_any(),
                                    None => view! { <span></span> }.into_any(),
                                }}
                            </Show>
                        }.into_any(),
                        Some(Err(err)) => view! {
                            <p class="text-sm text-rose-300">{err.to_string()}</p>
                        }.into_any(),
                        None => view! { <span></span> }.into_any(),
                    }}
                </Show>
            </Show>
        </section>
    }
}

#[component]
fn SummaryCard(
    summary: SummaryListItem,
    delete_action: Action<i64, Result<(), ServerFnError>>,
) -> impl IntoView {
    let summary_id = summary.id;
    let status_class = match summary.status.as_str() {
        "pending" | "processing" => "border-blue-500/40 bg-blue-500/5",
        "completed" => {
            "border-slate-800 bg-slate-900/50 hover:border-slate-600 hover:bg-slate-900/70"
        }
        "failed" => "border-rose-500/40 bg-rose-500/5",
        _ => "border-slate-800 bg-slate-900/50",
    };
    let is_clickable = summary.status == "completed";

    view! {
        <div class=format!("rounded-2xl border p-6 space-y-3 transition-colors block {}", status_class)>
            {if is_clickable {
                view! {
                    <a href=format!("/summaries/{}", summary.id) class="block">
                        <h3 class="text-lg font-semibold text-white">{summary.title.clone()}</h3>
                    </a>
                }.into_any()
            } else {
                view! {
                    <h3 class="text-lg font-semibold text-white">{summary.title.clone()}</h3>
                }.into_any()
            }}
            {summary.description.as_ref().map(|desc| view! {
                <p class="text-sm text-slate-400">{desc.clone()}</p>
            })}
            <div class="flex items-center justify-between">
                <div class="flex items-center gap-2">
                    {match summary.status.as_str() {
                        "pending" | "processing" => view! {
                            <>
                                <span class="h-3 w-3 animate-spin rounded-full border-2 border-blue-400 border-t-transparent"></span>
                                <span class="text-xs text-blue-300">"Generating..."</span>
                            </>
                        }.into_any(),
                        "failed" => view! {
                            <span class="text-xs text-rose-300">"Generation failed"</span>
                        }.into_any(),
                        _ => view! {
                            <span class="text-xs text-slate-500">
                                {summary.segment_label.as_ref().map(|l| format!("{} • ", l)).unwrap_or_default()}
                                {summary.created_at.clone()}
                            </span>
                        }.into_any(),
                    }}
                </div>
                <button
                    class="rounded-full border border-rose-800 px-3 py-1 text-xs text-rose-300 hover:border-rose-600"
                    on:click=move |_| {
                        if window().confirm_with_message(&format!("Delete summary '{}'?", summary.title)).unwrap_or(false) {
                            delete_action.dispatch(summary_id);
                        }
                    }
                >
                    "Delete"
                </button>
            </div>
        </div>
    }
}
