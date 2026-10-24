use russh::client::AuthResult;
use russh::MethodKind;
use zeroize::Zeroizing;

use super::password_cache::{PasswordCache, PasswordCacheKey};
use crate::connect_attempt::ConnectAttempt;

pub(super) struct PasswordAnswer {
    pub password: Zeroizing<String>,
    pub remember: bool,
}

pub(super) trait PasswordAuthIo {
    async fn authenticate(&mut self, password: String) -> Result<AuthResult, String>;
    async fn prompt(&mut self) -> Result<PasswordAnswer, String>;
    fn attempt(&self) -> Option<&ConnectAttempt>;
}

pub(super) async fn authenticate_with_cache(
    io: &mut impl PasswordAuthIo,
    cache: &PasswordCache,
    key: PasswordCacheKey,
    generation: u64,
    supplied_password: String,
) -> Result<AuthResult, String> {
    let mut lease = cache.checkout(key, generation);
    if !supplied_password.is_empty() {
        return io.authenticate(supplied_password).await;
    }
    if let Some(password) = lease.password.take() {
        let result = io.authenticate(password.to_string()).await;
        match result {
            Ok(AuthResult::Success) => return Ok(AuthResult::Success),
            Ok(AuthResult::Failure {
                remaining_methods,
                partial_success,
            }) => {
                cache.reject(&mut lease);
                if partial_success || !remaining_methods.contains(&MethodKind::Password) {
                    return Ok(AuthResult::Failure {
                        remaining_methods,
                        partial_success,
                    });
                }
            }
            Err(error) => {
                cache.reject(&mut lease);
                return Err(error);
            }
        }
    }
    let answer = io.prompt().await?;
    let result = io.authenticate(answer.password.to_string()).await?;
    if answer.remember && matches!(result, AuthResult::Success) {
        let remember = || cache.remember(&lease, answer.password);
        if let Some(attempt) = io.attempt() {
            attempt.if_active(remember);
        } else {
            remember();
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect_attempt::ConnectAttemptState;
    use std::collections::VecDeque;

    struct TestIo {
        results: VecDeque<Result<AuthResult, String>>,
        answer: Option<PasswordAnswer>,
        sent: Vec<String>,
        prompts: usize,
        on_auth: Option<Box<dyn FnOnce()>>,
        attempt: Option<ConnectAttempt>,
    }

    impl TestIo {
        fn new(remember: bool, results: Vec<Result<AuthResult, String>>) -> Self {
            Self {
                results: results.into(),
                answer: Some(PasswordAnswer {
                    password: Zeroizing::new("fresh".into()),
                    remember,
                }),
                sent: Vec::new(),
                prompts: 0,
                on_auth: None,
                attempt: None,
            }
        }
    }

    impl PasswordAuthIo for TestIo {
        async fn authenticate(&mut self, password: String) -> Result<AuthResult, String> {
            self.sent.push(password);
            if let Some(action) = self.on_auth.take() {
                action();
            }
            self.results
                .pop_front()
                .expect("unexpected authentication retry")
        }
        async fn prompt(&mut self) -> Result<PasswordAnswer, String> {
            self.prompts += 1;
            self.answer.take().ok_or_else(|| "cancelled".into())
        }
        fn attempt(&self) -> Option<&ConnectAttempt> {
            self.attempt.as_ref()
        }
    }

    fn key() -> PasswordCacheKey {
        PasswordCacheKey {
            host: "router.test".into(),
            port: 22,
            username: "admin".into(),
            host_key: "key".into(),
        }
    }

    fn rejected() -> Result<AuthResult, String> {
        Ok(AuthResult::Failure {
            remaining_methods: russh::MethodSet::from(&[MethodKind::Password][..]),
            partial_success: false,
        })
    }

    async fn run(io: &mut TestIo, cache: &PasswordCache) -> Result<AuthResult, String> {
        authenticate_with_cache(io, cache, key(), cache.generation(), String::new()).await
    }

    fn seed(cache: &PasswordCache) {
        let lease = cache.checkout(key(), cache.generation());
        cache.remember(&lease, Zeroizing::new("old".into()));
    }

    #[tokio::test]
    async fn opt_in_only_remembers_success_and_reuses_across_clients() {
        for remember in [false, true] {
            let cache = PasswordCache::default();
            let mut first = TestIo::new(remember, vec![Ok(AuthResult::Success)]);
            assert!(matches!(
                run(&mut first, &cache).await.unwrap(),
                AuthResult::Success
            ));
            let mut next = TestIo::new(false, vec![Ok(AuthResult::Success)]);
            run(&mut next, &cache.clone()).await.unwrap();
            assert_eq!(next.prompts, usize::from(!remember));
            assert_eq!(next.sent, ["fresh"]);
        }
        let cache = PasswordCache::default();
        run(&mut TestIo::new(true, vec![rejected()]), &cache)
            .await
            .unwrap();
        assert!(cache.checkout(key(), cache.generation()).password.is_none());
    }

    #[tokio::test]
    async fn rejected_cache_prompts_once_and_can_replace_or_forget() {
        for remember in [false, true] {
            let cache = PasswordCache::default();
            seed(&cache);
            let mut io = TestIo::new(remember, vec![rejected(), Ok(AuthResult::Success)]);
            run(&mut io, &cache).await.unwrap();
            assert_eq!(io.sent, ["old", "fresh"]);
            assert_eq!(io.prompts, 1);
            assert_eq!(
                cache.checkout(key(), cache.generation()).password.is_some(),
                remember
            );
        }
        let cache = PasswordCache::default();
        seed(&cache);
        let mut io = TestIo::new(true, vec![rejected(), rejected()]);
        assert!(matches!(
            run(&mut io, &cache).await.unwrap(),
            AuthResult::Failure { .. }
        ));
        assert_eq!(io.prompts, 1);
        assert!(cache.checkout(key(), cache.generation()).password.is_none());
    }

    #[tokio::test]
    async fn disconnected_cache_is_evicted_without_reconnecting() {
        let cache = PasswordCache::default();
        seed(&cache);
        let mut io = TestIo::new(true, vec![Err("disconnected".into())]);
        assert_eq!(run(&mut io, &cache).await.unwrap_err(), "disconnected");
        assert_eq!(io.prompts, 0);
        assert!(cache.checkout(key(), cache.generation()).password.is_none());
    }

    #[tokio::test]
    async fn cancellation_or_clear_during_authentication_cannot_remember_late_success() {
        for cancel in [false, true] {
            let cache = PasswordCache::default();
            let state = ConnectAttemptState::new("cancelled", "duplicate");
            let mut io = TestIo::new(true, vec![Ok(AuthResult::Success)]);
            io.attempt = Some(state.register("request".into()).unwrap());
            let clone = cache.clone();
            io.on_auth = Some(Box::new(move || {
                if cancel {
                    state.cancel("request");
                } else {
                    clone.clear();
                }
            }));
            run(&mut io, &cache).await.unwrap();
            assert!(cache.checkout(key(), cache.generation()).password.is_none());
        }
    }

    #[tokio::test]
    async fn cancelled_prompt_and_partial_success_never_remember() {
        let cache = PasswordCache::default();
        let mut io = TestIo::new(true, vec![]);
        io.answer = None;
        assert_eq!(run(&mut io, &cache).await.unwrap_err(), "cancelled");
        let mut io = TestIo::new(
            true,
            vec![Ok(AuthResult::Failure {
                remaining_methods: russh::MethodSet::from(&[MethodKind::KeyboardInteractive][..]),
                partial_success: true,
            })],
        );
        run(&mut io, &cache).await.unwrap();
        assert!(cache.checkout(key(), cache.generation()).password.is_none());
    }

    #[test]
    fn clear_restart_and_identity_changes_prevent_reuse() {
        let cache = PasswordCache::default();
        seed(&cache);
        let original = key();
        for different in [
            PasswordCacheKey {
                host: "192.0.2.1".into(),
                ..key()
            },
            PasswordCacheKey {
                port: 2222,
                ..key()
            },
            PasswordCacheKey {
                username: "other".into(),
                ..key()
            },
            PasswordCacheKey {
                host_key: "new-key".into(),
                ..key()
            },
        ] {
            assert!(cache
                .checkout(different, cache.generation())
                .password
                .is_none());
        }
        let stale = cache.checkout(original, cache.generation());
        cache.clear();
        cache.remember(&stale, Zeroizing::new("late".into()));
        assert!(cache.checkout(key(), cache.generation()).password.is_none());
        assert!(PasswordCache::default()
            .checkout(key(), 0)
            .password
            .is_none());
    }

    #[test]
    fn stale_failure_and_success_cannot_replace_newer_credentials() {
        let cache = PasswordCache::default();
        seed(&cache);
        let mut stale = cache.checkout(key(), 0);
        let mut newer = cache.checkout(key(), 0);
        cache.reject(&mut newer);
        cache.remember(&newer, Zeroizing::new("new".into()));
        cache.reject(&mut stale);
        cache.remember(&stale, Zeroizing::new("stale".into()));
        assert_eq!(
            cache
                .checkout(key(), 0)
                .password
                .as_deref()
                .map(|s| s.as_str()),
            Some("new")
        );
    }
}
