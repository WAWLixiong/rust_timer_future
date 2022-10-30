use std::thread;
use {
    futures::{
        future::{BoxFuture, FutureExt},
        task::{waker_ref, ArcWake},
    },
    std::{
        future::Future,
        sync::mpsc::{sync_channel, Receiver, SyncSender},
        sync::{Arc, Mutex},
        task::{Context},
        time::Duration,
    },
    timer_future::TimerFuture,
};


struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}


#[derive(Clone)]
struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    fn spawn(&self, future: impl Future<Output=()> + 'static + Send) {
        let future = future.boxed();
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });
        println!("[{:?}] 将 Future 组成 Task, 放入 Channel ...", thread::current().id());
        self.task_sender.send(task).expect("too many tasks queued");
    }
}


struct Task {
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    task_sender: SyncSender<Arc<Task>>,
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        println!("[{:?}] wake_by_ref", thread::current().id());
        let cloned = arc_self.clone();
        arc_self.task_sender.send(cloned).expect("too many tasks queued");
    }
}


fn new_executor_and_spawner() -> (Executor, Spawner) {
    const MAX_QUEUED_TASKS: usize = 10_000;
    let (task_sender, ready_queue) = sync_channel(MAX_QUEUED_TASKS);
    println!("[{:?}] 生成 Executor 和 Spawner (含发送端，接收端) ...", thread::current().id());
    (Executor { ready_queue }, Spawner { task_sender })
}


impl Executor {
    fn run(&self) {
        println!("[{:?}] Executor running ...", thread::current().id());
        while let Ok(task) = self.ready_queue.recv() {
            println!("[{:?}] 接收到任务 ...", thread::current().id());
            let mut future_slot = task.future.lock().unwrap();
            if let Some(mut future) = future_slot.take() {
                println!("[{:?}] 从任务中取得 Future ...", thread::current().id());
                let waker = waker_ref(&task);
                println!("[{:?}] 获得 waker by ref ...", thread::current().id());
                let context = &mut Context::from_waker(&*waker);
                println!("[{:?}] 获得 context 准备 poll() ...", thread::current().id());
                if future.as_mut().poll(context).is_pending() {
                    *future_slot = Some(future);
                    println!("[{:?}] Poll:Pending ====", thread::current().id());
                } else {
                    println!("[{:?}] Poll:Ready ====", thread::current().id());
                }
            }
        }
        println!("[{:?}] Executor run 结束", thread::current().id());
    }
}


fn main() {
    let (executor, spawner) = new_executor_and_spawner();
    spawner.spawn(async {
        println!("howdy!");
        TimerFuture::new(Duration::new(2, 0)).await;
        println!("done!");
    });
    println!("[{:?}] Drop Spawner!", thread::current().id());
    drop(spawner);
    executor.run();
}