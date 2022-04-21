//! This module provide the ability to easily hot reload but the config and apps
//! list on file system changes.

use std::mem;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::{Config, CFG_PATH};
use crossbeam_channel::{Receiver, Select, Sender};
use notify::{
    watcher, DebouncedEvent::*, RecommendedWatcher, RecursiveMode, Watcher,
};

/// Hot reload callback function
pub type Callback = Box<dyn Fn() + Send + 'static>;

#[derive(Default)]
pub struct HotReloadBuilder<'config> {
    config: Option<Callback>,
    apps: Option<(&'config Config, Callback)>,
}

pub fn build<'config>() -> HotReloadBuilder<'config> {
    HotReloadBuilder::default()
}

impl<'config> HotReloadBuilder<'config> {
    pub fn config(
        &mut self,
        callback: impl Fn() + Send + 'static,
    ) -> &mut Self {
        self.config = Some(Box::new(callback));
        self
    }

    pub fn apps(
        &mut self,
        config: &'config Config,
        callback: impl Fn() + Send + 'static,
    ) -> &mut Self {
        self.apps = Some((config, Box::new(callback)));
        self
    }

    pub fn start(self) -> notify::Result<HotReloadHandle> {
        let opt = match (self.config, self.apps) {
            (Some(config_cb), Some((config, desktop_cb))) => {
                HotReloadType::Both(config, config_cb, desktop_cb)
            }
            (Some(config_cb), None) => HotReloadType::Config(config_cb),
            (None, Some((config, desktop_cb))) => {
                HotReloadType::Apps(config, desktop_cb)
            }
            (None, None) => HotReloadType::None,
        };
        hot_reload(opt)
    }
}

enum HotReloadType<'config> {
    Config(Callback),
    Apps(&'config Config, Callback),
    Both(&'config Config, Callback, Callback),
    None,
}

pub struct HotReloadHandle {
    config_watcher: Option<RecommendedWatcher>,
    apps_watcher: Option<RecommendedWatcher>,
    join_handles: Vec<JoinHandle<()>>,
    current_app_paths: Vec<PathBuf>,
}

impl HotReloadHandle {
    // TODO this can't be called in the config callback because the
    // HotReloadHandle hasn't been created yet
    pub fn config_changes(&mut self, config: &Config) -> notify::Result<()> {
        if let Some(watcher) = &mut self.apps_watcher {
            for path in self.current_app_paths.drain(..) {
                watcher.unwatch(path)?;
            }
            for path in config.app_paths.clone() {
                watcher.watch(&path, RecursiveMode::Recursive)?;
                self.current_app_paths.push(path);
            }
        }
        Ok(())
    }
}

impl Drop for HotReloadHandle {
    fn drop(&mut self) {
        mem::drop(self.config_watcher.take());
        mem::drop(self.apps_watcher.take());
        for handle in self.join_handles.drain(..) {
            handle.join().unwrap();
        }
    }
}

fn hot_reload(opt: HotReloadType<'_>) -> notify::Result<HotReloadHandle> {
    match opt {
        HotReloadType::Config(callback) => {
            let (mut config_watcher, join_handle, recv) = new_watcher()?;
            set_config_watcher(&mut config_watcher)?;
            let cb_join_handle = setup_handlers(vec![(recv, callback)]);
            Ok(HotReloadHandle {
                config_watcher: Some(config_watcher),
                apps_watcher: None,
                join_handles: vec![join_handle, cb_join_handle],
                current_app_paths: vec![],
            })
        }
        HotReloadType::Apps(config, callback) => {
            let (mut apps_watcher, join_handle, recv) =
                new_watcher()?;
            set_apps_watcher(&mut apps_watcher, &config.app_paths)?;
            let cb_join_handle = setup_handlers(vec![(recv, callback)]);
            Ok(HotReloadHandle {
                config_watcher: None,
                apps_watcher: Some(apps_watcher),
                join_handles: vec![join_handle, cb_join_handle],
                current_app_paths: config.app_paths.clone(),
            })
        }
        HotReloadType::Both(config, config_cb, desktop_cb) => {
            let (mut config_watcher, config_join_handle, config_recv) =
                new_watcher()?;
            set_config_watcher(&mut config_watcher)?;
            let (mut apps_watcher, desktop_join_handle, desktop_recv) =
                new_watcher()?;
            set_apps_watcher(&mut apps_watcher, &config.app_paths)?;
            let cb_join_handle = setup_handlers(vec![
                (config_recv, config_cb),
                (desktop_recv, desktop_cb),
            ]);
            Ok(HotReloadHandle {
                config_watcher: Some(config_watcher),
                apps_watcher: Some(apps_watcher),
                join_handles: vec![
                    config_join_handle,
                    desktop_join_handle,
                    cb_join_handle,
                ],
                current_app_paths: config.app_paths.clone(),
            })
        }
        HotReloadType::None => panic!("HotReloadBuilder::start require that a config or desktop file reload has been set"),
    }
}

fn setup_handlers(
    recv_callback: Vec<(Receiver<()>, Callback)>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut sel = Select::new();
        for r in &recv_callback {
            sel.recv(&r.0);
        }
        loop {
            let idx = sel.ready();
            match recv_callback[idx].0.recv() {
                Ok(_) => recv_callback[idx].1(),
                _ => break,
            }
        }
    })
}

fn new_watcher(
) -> notify::Result<(RecommendedWatcher, JoinHandle<()>, Receiver<()>)> {
    let (sender, recv) = crossbeam_channel::unbounded();
    let (watcher, join_handle) = start_watcher(sender)?;
    Ok((watcher, join_handle, recv))
}

fn set_config_watcher(watcher: &mut RecommendedWatcher) -> notify::Result<()> {
    watcher.watch(&*CFG_PATH, RecursiveMode::NonRecursive)
}

fn set_apps_watcher(
    watcher: &mut RecommendedWatcher,
    app_paths: &[PathBuf],
) -> notify::Result<()> {
    for path in app_paths {
        if path.exists() {
            if let Err(e) = watcher.watch(path, RecursiveMode::Recursive) {
                log::warn!(
                    "Failed to set watcher for dir {}: {}",
                    path.display(),
                    e
                );
            }
        }
    }
    Ok(())
}

fn start_watcher(
    sender: Sender<()>,
) -> notify::Result<(RecommendedWatcher, JoinHandle<()>)> {
    let (tx, rx) = mpsc::channel();
    let watcher = match watcher(tx, Duration::from_secs(1)) {
        Ok(watcher) => watcher,
        Err(e) => {
            return Err(e);
        }
    };
    let join_handle = thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            match event {
                Write(_) | Remove(_) | Rename(_, _) | Create(_) => {
                    sender.send(()).unwrap();
                }
                _ => {}
            }
        }
    });
    Ok((watcher, join_handle))
}
