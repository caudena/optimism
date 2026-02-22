package bpo2

import (
	"testing"

	"github.com/ethereum-optimism/optimism/op-acceptance-tests/tests/fusaka"
	jovian "github.com/ethereum-optimism/optimism/op-acceptance-tests/tests/jovian"
	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-devstack/stack"
	"github.com/ethereum-optimism/optimism/op-devstack/sysgo"
	"github.com/ethereum/go-ethereum/params/forks"
)

func newSystem(gt *testing.T) *presets.Minimal {
	resetEnv := fusaka.ConfigureDevstackEnvVars()
	gt.Cleanup(resetEnv)
	t := devtest.SerialT(gt)
	return presets.NewMinimal(t,
		stack.MakeCommon(sysgo.WithDeployerOptions(
			sysgo.WithJovianAtGenesis,
			sysgo.WithDefaultBPOBlobSchedule,
			sysgo.WithForkAtL1Genesis(forks.BPO2),
		)),
	)
}

func TestDAFootprint(gt *testing.T) {
	jovian.TestDAFootprint(newSystem(gt))
}

func TestMinBaseFee(gt *testing.T) {
	jovian.TestMinBaseFee(newSystem(gt))
}

func TestOperatorFee(gt *testing.T) {
	jovian.TestOperatorFee(newSystem(gt))
}
