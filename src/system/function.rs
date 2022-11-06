use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::{
    System, SystemMeta, SystemParam, SystemParamFetch, SystemParamItem, SystemParamState, World,
    WorldId,
};

pub trait IntoSystem<In, Out, Params>: Sized {
    type System: System<In = In, Out = Out>;

    fn into_system(self) -> Self::System;
}

impl<In, Out, S> IntoSystem<In, Out, ()> for S
where
    S: System<In = In, Out = Out>,
{
    type System = S;

    #[inline]
    fn into_system(self) -> Self::System {
        self
    }
}

pub struct FunctionSystem<In, Out, Param, Marker, F>
where
    Param: SystemParam,
{
    func: F,
    param_state: Option<Param::Fetch>,
    meta: SystemMeta,
    world_id: Option<WorldId>,
    _marker: PhantomData<fn() -> (In, Out, Marker)>,
}

impl<In, Out, Params, Marker, F> IntoSystem<In, Out, (Params, Marker)> for F
where
    In: 'static,
    Out: 'static,
    Params: SystemParam + 'static,
    Marker: 'static,
    F: SystemParamFunction<In, Out, Params, Marker> + Send + Sync + 'static,
{
    type System = FunctionSystem<In, Out, Params, Marker, F>;

    #[inline]
    fn into_system(self) -> Self::System {
        FunctionSystem {
            func: self,
            param_state: None,
            meta: SystemMeta::new::<Params>(),
            world_id: None,
            _marker: PhantomData,
        }
    }
}

impl<In, Out, Param, Marker, F> System for FunctionSystem<In, Out, Param, Marker, F>
where
    In: 'static,
    Out: 'static,
    Param: SystemParam + 'static,
    Marker: 'static,
    F: SystemParamFunction<In, Out, Param, Marker> + Send + Sync + 'static,
{
    type In = In;
    type Out = Out;

    #[inline]
    fn meta(&self) -> &SystemMeta {
        &self.meta
    }

    #[inline]
    unsafe fn meta_mut(&mut self) -> &mut SystemMeta {
        &mut self.meta
    }

    #[inline]
    fn init(&mut self, world: &mut World) {
        self.world_id = Some(world.id());
        self.meta.last_change_tick = world.change_tick();
        self.param_state = Some(<Param::Fetch as SystemParamState>::init(
            world,
            &mut self.meta,
        ));
    }

    #[inline]
    unsafe fn run(&mut self, input: Self::In, world: &World) -> Self::Out {
        let change_tick = world.increment_change_tick();

        let params = unsafe {
            <Param as SystemParam>::Fetch::get_param(
                self.param_state.as_mut().unwrap(),
                &self.meta,
                world,
                change_tick,
            )
        };

        let out = self.func.run(input, params);

        self.meta.last_change_tick = change_tick;

        out
    }

    #[inline]
    fn apply(&mut self, world: &mut World) {
        let param_state = self.param_state.as_mut().unwrap();
        param_state.apply(world);
    }
}

#[doc(hidden)]
pub struct InputMarker;

pub struct In<T> {
    value: T,
}

impl<T> In<T> {
    #[inline]
    pub fn new(value: T) -> Self {
        Self { value }
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> From<T> for In<T> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Deref for In<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for In<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

pub trait SystemParamFunction<In, Out, Params: SystemParam, Marker>: Send + Sync + 'static {
    fn run(&mut self, input: In, item: SystemParamItem<Params>) -> Out;
}

macro_rules! impl_system_param_function {
    (@ $($param:ident),*) => {
        #[allow(non_snake_case)]
        impl<Out, Func, $($param: SystemParam),*> SystemParamFunction<(), Out, ($($param,)*), ()> for Func
        where
            Out: 'static,
            Func: Send + Sync + 'static,
            Func: FnMut($($param),*) -> Out,
            Func: FnMut($(SystemParamItem<$param>),*) -> Out,
        {
            #[inline]
            fn run(&mut self, _input: (), item: SystemParamItem<($($param,)*)>) -> Out {
                let ($($param,)*) = item;
                (self)($($param),*)
            }
        }

        #[allow(non_snake_case)]
        impl<Input, Out, Func, $($param: SystemParam),*> SystemParamFunction<Input, Out, ($($param,)*), InputMarker> for Func
        where
            Out: 'static,
            Func: Send + Sync + 'static,
            Func: FnMut(In<Input>, $($param),*) -> Out,
            Func: FnMut(In<Input>, $(SystemParamItem<$param>),*) -> Out,
        {
            #[inline]
            fn run(&mut self, input: Input, item: SystemParamItem<($($param,)*)>) -> Out {
                let ($($param,)*) = item;
                (self)(In::new(input), $($param),*)
            }
        }
    };
    ($start:ident $(,$ident:ident)*) => {
        impl_system_param_function!(@ $start $(,$ident)*);
        impl_system_param_function!($($ident),*);
    };
    () => {
        impl_system_param_function!(@);
    };
}

impl_system_param_function!(A, B, C, D, E, F, G, H, I, J, K, L);

#[cfg(test)]
mod tests {
    use crate::*;

    fn test_system(query: Query<&i32>) {
        for item in query.iter() {
            assert_eq!(*item, 10);
        }
    }

    #[test]
    fn test_system_param_function() {
        let mut world = World::new();

        world.spawn().insert(10);

        let mut system = test_system.into_system();

        system.init(&mut world);
        unsafe { system.run((), &mut world) };
        system.apply(&mut world);
    }
}
