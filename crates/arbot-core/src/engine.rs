use tokio::sync::broadcast::{self, Sender};
use tokio::task::JoinSet;
use tokio_stream::StreamExt;
use tracing::{error, info};

use crate::types::{Collector, Executor, Strategy};

/// The main engine of Artemis. This struct is responsible for orchestrating the
/// data flow between collectors, strategies, and executors.
pub struct Engine<E, A> {
    /// The set of collectors that the engine will use to collect events.   // 收集器 用于收集事件
    collectors: Vec<Box<dyn Collector<E>>>,

    /// The set of strategies that the engine will use to process events.   // 策略 用于处理事件
    strategies: Vec<Box<dyn Strategy<E, A>>>,

    /// The set of executors that the engine will use to execute actions.   // 执行器 用于执行动作
    executors: Vec<Box<dyn Executor<A>>>,
}

impl<E, A> Engine<E, A> {
    pub fn new() -> Self {
        Self {
            collectors: vec![],
            strategies: vec![],
            executors: vec![],
        }
    }
}

impl<E, A> Default for Engine<E, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E, A> Engine<E, A> 
    where
    E: Send + Clone + 'static + std::fmt::Debug,
    A: Send + Clone + 'static + std::fmt::Debug,
{
    /// Adds a collector to be used by the engine.  // 添加收集器 用于引擎
    pub fn add_collector(&mut self, collector: Box<dyn Collector<E>>) {
        self.collectors.push(collector);
    }

    /// Adds a strategy to be used by the engine.   // 添加策略 用于引擎
    pub fn add_strategy(&mut self, strategy: Box<dyn Strategy<E, A>>) {
        self.strategies.push(strategy);
    }

    /// Adds an executor to be used by the engine.  // 添加执行器 用于引擎
    pub fn add_executor(&mut self, executor: Box<dyn Executor<A>>) {
        self.executors.push(executor);
    }

    /// Core run loop 引擎
    /// 这个函数将为每个收集器、策略和执行器生成一个线程。
    /// 然后它将协调它们之间的数据流
    pub async fn run(self) -> Result<JoinSet<()>, Box<dyn std::error::Error>> {
        let (event_sender, _): (Sender<E>, _) = broadcast::channel(512);
        let (action_sender, _): (Sender<A>, _) = broadcast::channel(512);

        let mut set = JoinSet::new();

        // 在单独的线程中 启动执行器
        for executor in self.executors {
            let mut action_receiver = action_sender.subscribe();                                // 动作接收
            set.spawn(async move {
                info!("starting executor... ");
                loop {
                    match action_receiver.recv().await {                                        // 接收动作
                        Ok(action) => match executor.execute(action).await {                    // 执行动作
                            Ok(_) => {}
                            Err(e) => error!("error executing action: {}", e),
                        },
                        Err(e) => error!("error receiving action: {}", e),
                    }
                }
            });
        }

        // 在单独的线程中 启动策略
        for mut strategy in self.strategies {
            let mut event_receiver = event_sender.subscribe();                                  // 事件接收者
            let action_sender = action_sender.clone();                                          // 动作发送者
            strategy.sync_state().await?;                                                       // 同步状态

            set.spawn(async move {
                info!("starting strategy... ");                                                 // 开始策略
                loop {
                    match event_receiver.recv().await {                                         // 接收事件
                        Ok(event) => {
                            if let Some(action) = strategy.process_event(event).await {         // 处理事件
                                match action_sender.send(action) {                              // 发送动作
                                    Ok(_) => {}
                                    Err(e) => error!("error sending action: {}", e),
                                }
                            }
                        }
                        Err(e) => error!("error receiving event: {}", e),
                    }
                }
            });
        }

        // 在单独的线程中 启动收集器
        for collector in self.collectors {                                              // 收集器
            let event_sender = event_sender.clone();                                    // 事件发送者
            set.spawn(async move {                                                      // 开始收集器
                info!("starting collector... ");
                let mut event_stream = collector.get_event_stream().await.unwrap();     // 获取事件流
                while let Some(event) = event_stream.next().await {                     // 获取事件
                    match event_sender.send(event) {                                    // 发送事件
                        Ok(_) => {}
                        Err(e) => error!("error sending event: {}", e),
                    }
                }
            });
        }

        Ok(set)
    }
}
