if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
function __ReusableV2_Inner_Decorator__(BaseClass) {
	Reflect.defineProperty(BaseClass.prototype, 'isReusable_', { get: () => true });
}
class ReuseParent extends ViewV2 {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
		super(parent, elmtId, extraInfo);
		this.n = 1;
		this.finalizeConstruction();
	}
	public resetStateVarsOnReuse(params: Object): void {
		this.n = 1;
	}
	@Local n: number;
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		{
			this.reuseOrCreateNewComponent({
				componentClass: ReuseKid,
				getParams: () => ({ title: `${this.n}` }),
				getReuseId: getUINativeModule().common.getApiTargetVersion() >= 26 ? () => 'ReuseKid' : () => 'ReuseKid',
				extraInfo: {
					page: 'entry/src/main/ets/pages/Index.ets',
					line: 8,
					col: 7
				}
			});
		}
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'ReuseParent';
	}
}
@__ReusableV2_Inner_Decorator__ class ReuseKid extends ViewV2 {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
		super(parent, elmtId, extraInfo);
		this.initParam('title', params && 'title' in params ? params.title : '');
		this.finalizeConstruction();
	}
	public resetStateVarsOnReuse(params: Object): void {
		this.resetParam('title', params && 'title' in params ? params.title : '');
	}
	@Param readonly title: string;
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(this.title);
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
registerNamedRoute(() => new ReuseParent(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
