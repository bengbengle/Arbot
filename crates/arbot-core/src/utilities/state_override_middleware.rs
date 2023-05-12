use async_trait::async_trait;
use ethers::{
    core::types::{transaction::eip2718::TypedTransaction, BlockId},
    providers::{spoof, CallBuilder, Middleware, MiddlewareError, RawCall},
    types::{Address, Bytes},
};
use thiserror::Error;

/// This custom middleware performs an ephemeral state override prior to executoring calls.
/// 自定义中间件，在执行调用之前执行临时状态覆盖。
#[derive(Debug)]
pub struct StateOverrideMiddleware<M> {
    /// The inner middleware
    /// 内部中间件
    inner: M,
    /// The state override set we use for calls  
    /// 我们 用于调用 的 状态覆盖 集
    state: spoof::State,
}

impl<M> StateOverrideMiddleware<M>
where
    M: Middleware,
{
    /// Creates an instance of StateOverrideMiddleware `ìnner` the inner Middleware
    /// 创建 StateOverrideMiddleware 的实例  `ìnner` 内部中间件
    pub fn new(inner: M) -> StateOverrideMiddleware<M> {
        Self {
            inner,
            state: spoof::state(),
        }
    }
}

#[async_trait]
impl<M> Middleware for StateOverrideMiddleware<M>
where
    M: Middleware,
{
    type Error = StateOverrideMiddlewareError<M>;
    type Provider = M::Provider;
    type Inner = M;

    fn inner(&self) -> &M {
        &self.inner
    }

    /// Performs a call with the state override.
    /// 使用状态覆盖执行调用
    async fn call(
        &self,
        tx: &TypedTransaction,
        block: Option<BlockId>,
    ) -> Result<Bytes, Self::Error> {
        let call_builder = CallBuilder::new(self.inner.provider(), tx);
        let call_builder = match block {
            Some(block) => call_builder.block(block),
            None => call_builder,
        };
        let call_builder = call_builder.state(&self.state);
        call_builder
            .await
            .map_err(StateOverrideMiddlewareError::from_provider_err)
    }
}

impl<M> StateOverrideMiddleware<M> {
    /// Adds a code override at a given address.
    /// 在给定地址 添加代码覆盖
    pub fn add_code_to_address(&mut self, address: Address, code: Bytes) {
        self.state.account(address).code(code);
    }

    /// Adds a code override at a random address, returning the address.
    /// 在随机地址 添加代码覆盖，并返回地址
    pub fn add_code(&mut self, code: Bytes) -> Address {
        let address = Address::random();
        self.state.account(address).code(code);
        address
    }
}

#[derive(Debug, Error)]
pub enum StateOverrideMiddlewareError<M: Middleware> {
    /// Thrown when the internal middleware errors
    /// 内部中间件错误时抛出
    #[error("{0}")]
    MiddlewareError(M::Error),
}

impl<M: Middleware> MiddlewareError for StateOverrideMiddlewareError<M> {
    type Inner = M::Error;

    fn from_err(src: M::Error) -> Self {
        StateOverrideMiddlewareError::MiddlewareError(src)
    }

    fn as_inner(&self) -> Option<&Self::Inner> {
        match self {
            StateOverrideMiddlewareError::MiddlewareError(e) => Some(e),
        }
    }
}
