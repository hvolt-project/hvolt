if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
class V2ArgsParent extends ViewV2 {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
		super(parent, elmtId, extraInfo);
		this.count = 1;
		this.finalizeConstruction();
	}
	public resetStateVarsOnReuse(params: Object): void {
		this.count = 1;
	}
	@Local count: number;
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new V2ArgsKid(this, {
						title: this.count + 1,
						onTap: () => {}
					}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 8,
						col: 7
					});
					ViewV2.create(componentCall);
					let paramsLambda = () => {
						return {
							title: this.count + 1,
							onTap: () => {}
						};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, { title: this.count + 1 });
				}
			}, { name: 'V2ArgsKid' });
		}
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'V2ArgsParent';
	}
}
class V2ArgsKid extends ViewV2 {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
		super(parent, elmtId, extraInfo);
		this.initParam('title', params && 'title' in params ? params.title : 0);
		this.onTap = 'onTap' in params ? params.onTap : () => {};
		this.finalizeConstruction();
	}
	public resetStateVarsOnReuse(params: Object): void {
		this.resetParam('title', params && 'title' in params ? params.title : 0);
		this.onTap = 'onTap' in params ? params.onTap : () => {};
	}
	@Param readonly title: number;
	@Event onTap: () => void;
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.title}`);
		}, Text);
		Text.pop();
	}
	public updateStateVars(params) {
		if (params === undefined) {
			return;
		}
		if ('title' in params) {
			this.updateParam('title', params.title);
		}
	}
	rerender() {
		this.updateDirtyElements();
	}
}
registerNamedRoute(() => new V2ArgsParent(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
