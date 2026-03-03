use leptos::prelude::*;

#[component]
pub fn Modal(
    /// Whether the modal is shown
    show: Signal<bool>,
    /// Callback to close the modal
    on_close: impl Fn() + 'static + Clone,
    /// Modal title
    title: String,
    /// Modal content
    children: Children,
    /// Optional max width class (default: "max-w-3xl")
    #[prop(optional)]
    max_width: Option<String>,
) -> impl IntoView {
    let max_width = max_width.unwrap_or_else(|| "max-w-3xl".to_string());
    let on_close_clone = on_close.clone();

    view! {
        <Show when=move || show.get()>
            <div
                class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 p-6"
                on:click=move |_| on_close_clone()
            >
                <div
                    class=format!("w-full {} rounded-2xl border border-slate-800 bg-slate-950 shadow-2xl", max_width)
                    on:click=move |ev| ev.stop_propagation()
                >
                    <div class="flex items-center justify-between border-b border-slate-800 px-6 py-4">
                        <h3 class="text-lg font-semibold text-white">{title}</h3>
                        <button
                            class="rounded-full border border-slate-700 px-4 py-1 text-xs text-slate-300 hover:border-slate-400"
                            on:click=move |_| on_close()
                        >
                            "Close"
                        </button>
                    </div>
                    <div class="px-6 py-5">
                        {children()}
                    </div>
                </div>
            </div>
        </Show>
    }
}
