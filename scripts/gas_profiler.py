#!/usr/bin/env python3
"""
Comprehensive gas profiling and analysis for XWork contract
Generates detailed reports with recommendations for gas limits
"""

import json
import subprocess
import sys
import time
from dataclasses import dataclass
from typing import Dict, List, Optional, Tuple
import argparse

@dataclass
class GasResult:
    operation: str
    gas_used: int
    gas_wanted: int
    tx_hash: str
    success: bool
    error: Optional[str] = None

@dataclass
class ProfileResult:
    operation: str
    min_gas: int
    max_gas: int
    avg_gas: int
    samples: int
    success_rate: float
    recommended_limit: int

class GasProfiler:
    def __init__(self, chain_binary="wasmd", chain_id="localnet", key="benchmark", node="tcp://localhost:26657"):
        self.chain_binary = chain_binary
        self.chain_id = chain_id
        self.key = key
        self.node = node
        self.contract_addr = None
        self.results: List[GasResult] = []
    
    def setup_contract(self, wasm_path: str) -> bool:
        """Store and instantiate contract"""
        try:
            print("📦 Storing contract...")
            store_cmd = [
                self.chain_binary, "tx", "wasm", "store", wasm_path,
                "--node", self.node, "--chain-id", self.chain_id,
                "--from", self.key, "--gas", "auto", "--gas-adjustment", "1.3",
                "--gas-prices", "0.025uxion", "-y", "--output", "json"
            ]
            
            result = subprocess.run(store_cmd, capture_output=True, text=True)
            if result.returncode != 0:
                print(f"❌ Failed to store contract: {result.stderr}")
                return False
            
            # Extract code ID
            store_output = json.loads(result.stdout)
            code_id = None
            for event in store_output.get("logs", [{}])[0].get("events", []):
                if event.get("type") == "store_code":
                    for attr in event.get("attributes", []):
                        if attr.get("key") == "code_id":
                            code_id = attr.get("value")
                            break
            
            if not code_id:
                print("❌ Could not extract code ID")
                return False
            
            print(f"✅ Contract stored with code ID: {code_id}")
            
            # Instantiate contract
            print("🚀 Instantiating contract...")
            init_msg = {
                "admin": self.key,
                "platform_fee_percent": 5,
                "min_escrow_amount": "1000",
                "dispute_period_days": 7,
                "max_job_duration_days": 365
            }
            
            instantiate_cmd = [
                self.chain_binary, "tx", "wasm", "instantiate", code_id,
                json.dumps(init_msg), "--label", "xwork-profiler",
                "--node", self.node, "--chain-id", self.chain_id,
                "--from", self.key, "--gas", "auto", "--gas-adjustment", "1.3",
                "--gas-prices", "0.025uxion", "-y", "--output", "json"
            ]
            
            result = subprocess.run(instantiate_cmd, capture_output=True, text=True)
            if result.returncode != 0:
                print(f"❌ Failed to instantiate contract: {result.stderr}")
                return False
            
            # Extract contract address
            instantiate_output = json.loads(result.stdout)
            for event in instantiate_output.get("logs", [{}])[0].get("events", []):
                if event.get("type") == "instantiate":
                    for attr in event.get("attributes", []):
                        if attr.get("key") == "_contract_address":
                            self.contract_addr = attr.get("value")
                            break
            
            if not self.contract_addr:
                print("❌ Could not extract contract address")
                return False
            
            print(f"✅ Contract instantiated at: {self.contract_addr}")
            return True
            
        except Exception as e:
            print(f"❌ Setup failed: {e}")
            return False
    
    def execute_and_measure(self, msg: dict, operation: str, amount: Optional[str] = None) -> GasResult:
        """Execute a message and measure gas usage"""
        try:
            cmd = [
                self.chain_binary, "tx", "wasm", "execute", self.contract_addr,
                json.dumps(msg), "--node", self.node, "--chain-id", self.chain_id,
                "--from", self.key, "--gas", "auto", "--gas-adjustment", "1.3",
                "--gas-prices", "0.025uxion", "-y", "--output", "json"
            ]
            
            if amount:
                cmd.extend(["--amount", amount])
            
            result = subprocess.run(cmd, capture_output=True, text=True)
            
            if result.returncode == 0:
                output = json.loads(result.stdout)
                gas_used = int(output.get("gas_used", 0))
                gas_wanted = int(output.get("gas_wanted", 0))
                tx_hash = output.get("txhash", "")
                
                return GasResult(
                    operation=operation,
                    gas_used=gas_used,
                    gas_wanted=gas_wanted,
                    tx_hash=tx_hash,
                    success=True
                )
            else:
                return GasResult(
                    operation=operation,
                    gas_used=0,
                    gas_wanted=0,
                    tx_hash="",
                    success=False,
                    error=result.stderr
                )
                
        except Exception as e:
            return GasResult(
                operation=operation,
                gas_used=0,
                gas_wanted=0,
                tx_hash="",
                success=False,
                error=str(e)
            )
    
    def profile_operation(self, msg_factory, operation: str, samples: int = 5, amount: Optional[str] = None) -> ProfileResult:
        """Profile an operation multiple times"""
        print(f"🔬 Profiling {operation} ({samples} samples)...")
        
        gas_values = []
        successes = 0
        
        for i in range(samples):
            msg = msg_factory(i)
            result = self.execute_and_measure(msg, f"{operation}_{i}", amount)
            self.results.append(result)
            
            if result.success:
                gas_values.append(result.gas_used)
                successes += 1
                print(f"  Sample {i+1}: {result.gas_used:,} gas")
            else:
                print(f"  Sample {i+1}: FAILED - {result.error}")
            
            time.sleep(1)  # Brief pause between samples
        
        if gas_values:
            min_gas = min(gas_values)
            max_gas = max(gas_values)
            avg_gas = sum(gas_values) // len(gas_values)
            # Add 20% safety margin for recommended limit
            recommended_limit = int(max_gas * 1.2)
        else:
            min_gas = max_gas = avg_gas = recommended_limit = 0
        
        success_rate = successes / samples
        
        return ProfileResult(
            operation=operation,
            min_gas=min_gas,
            max_gas=max_gas,
            avg_gas=avg_gas,
            samples=samples,
            success_rate=success_rate,
            recommended_limit=recommended_limit
        )
    
    def run_comprehensive_profile(self) -> List[ProfileResult]:
        """Run comprehensive gas profiling for all major operations"""
        profiles = []
        
        # 1. PostJob (paid)
        def post_job_paid_factory(i):
            return {
                "post_job": {
                    "title": f"Paid Job {i}",
                    "description": "A paid job for gas profiling analysis",
                    "budget": "5000",
                    "category": "Development",
                    "skills_required": ["Rust", "CosmWasm"],
                    "duration_days": 30,
                    "documents": None,
                    "milestones": None
                }
            }
        profiles.append(self.profile_operation(post_job_paid_factory, "PostJob_Paid"))
        
        # 2. PostJob (free)
        def post_job_free_factory(i):
            return {
                "post_job": {
                    "title": f"Free Job {i}",
                    "description": "A free job for gas profiling",
                    "budget": "0",
                    "category": "Community",
                    "skills_required": ["Writing"],
                    "duration_days": 14,
                    "documents": None,
                    "milestones": None
                }
            }
        profiles.append(self.profile_operation(post_job_free_factory, "PostJob_Free"))
        
        # 3. SubmitProposal
        def submit_proposal_factory(i):
            return {
                "submit_proposal": {
                    "job_id": 0,  # Use first paid job
                    "cover_letter": f"Proposal {i} for comprehensive gas analysis testing",
                    "delivery_time_days": 25,
                    "milestones": None
                }
            }
        profiles.append(self.profile_operation(submit_proposal_factory, "SubmitProposal"))
        
        # 4. AcceptProposal (one sample)
        def accept_proposal_factory(i):
            return {
                "accept_proposal": {
                    "job_id": 0,
                    "proposal_id": 0
                }
            }
        profiles.append(self.profile_operation(accept_proposal_factory, "AcceptProposal", samples=1))
        
        # 5. CreateEscrow
        def create_escrow_factory(i):
            return {
                "create_escrow": {
                    "job_id": 0
                }
            }
        profiles.append(self.profile_operation(create_escrow_factory, "CreateEscrow", samples=1, amount="5000uxion"))
        
        # 6. CompleteJob
        def complete_job_factory(i):
            return {
                "complete_job": {
                    "job_id": 0
                }
            }
        profiles.append(self.profile_operation(complete_job_factory, "CompleteJob", samples=1))
        
        return profiles
    
    def generate_report(self, profiles: List[ProfileResult]) -> str:
        """Generate a comprehensive gas usage report"""
        report = []
        report.append("# XWork Contract Gas Usage Report")
        report.append(f"Generated: {time.strftime('%Y-%m-%d %H:%M:%S')}")
        report.append("")
        
        # Summary table
        report.append("## Gas Usage Summary")
        report.append("")
        report.append("| Operation | Min Gas | Max Gas | Avg Gas | Success Rate | Recommended Limit |")
        report.append("|-----------|---------|---------|---------|--------------|-------------------|")
        
        for profile in profiles:
            report.append(f"| {profile.operation} | {profile.min_gas:,} | {profile.max_gas:,} | {profile.avg_gas:,} | {profile.success_rate:.1%} | {profile.recommended_limit:,} |")
        
        report.append("")
        
        # Detailed analysis
        report.append("## Detailed Analysis")
        report.append("")
        
        for profile in profiles:
            report.append(f"### {profile.operation}")
            report.append(f"- **Samples**: {profile.samples}")
            report.append(f"- **Success Rate**: {profile.success_rate:.1%}")
            report.append(f"- **Gas Range**: {profile.min_gas:,} - {profile.max_gas:,}")
            report.append(f"- **Average**: {profile.avg_gas:,}")
            report.append(f"- **Recommended Limit**: {profile.recommended_limit:,}")
            
            if profile.success_rate < 1.0:
                report.append(f"- ⚠️ **Warning**: Some samples failed")
            
            report.append("")
        
        # Recommendations
        report.append("## Deployment Recommendations")
        report.append("")
        report.append("### Gas Limits for Frontend")
        report.append("```typescript")
        report.append("const GAS_LIMITS = {")
        for profile in profiles:
            safe_limit = profile.recommended_limit
            report.append(f"  {profile.operation.upper()}: {safe_limit},")
        report.append("};")
        report.append("```")
        report.append("")
        
        # Cost analysis
        report.append("### Cost Analysis (at 0.025 uxion per gas)")
        report.append("")
        for profile in profiles:
            cost_uxion = profile.avg_gas * 0.025
            report.append(f"- **{profile.operation}**: ~{cost_uxion:.2f} uxion ({profile.avg_gas:,} gas)")
        
        report.append("")
        report.append("### Optimization Opportunities")
        
        # Find high gas operations
        high_gas_ops = [p for p in profiles if p.avg_gas > 200000]
        if high_gas_ops:
            report.append("")
            report.append("**High gas operations (>200k gas):**")
            for op in high_gas_ops:
                report.append(f"- {op.operation}: {op.avg_gas:,} gas")
            report.append("")
            report.append("Consider optimizing these operations for better user experience.")
        else:
            report.append("- All operations are reasonably efficient (<200k gas)")
        
        return "\n".join(report)

def main():
    parser = argparse.ArgumentParser(description="Gas profiler for XWork contract")
    parser.add_argument("--wasm", default="./artifacts/xworks_freelance_contract.wasm", help="Path to WASM file")
    parser.add_argument("--chain-binary", default="wasmd", help="Chain binary name")
    parser.add_argument("--chain-id", default="localnet", help="Chain ID")
    parser.add_argument("--key", default="benchmark", help="Key name")
    parser.add_argument("--node", default="tcp://localhost:26657", help="Node URL")
    parser.add_argument("--samples", type=int, default=3, help="Number of samples per operation")
    parser.add_argument("--output", default="gas_report.md", help="Output report file")
    
    args = parser.parse_args()
    
    profiler = GasProfiler(args.chain_binary, args.chain_id, args.key, args.node)
    
    print("🚀 Starting comprehensive gas profiling...")
    
    # Setup contract
    if not profiler.setup_contract(args.wasm):
        sys.exit(1)
    
    # Run profiling
    profiles = profiler.run_comprehensive_profile()
    
    # Generate report
    report = profiler.generate_report(profiles)
    
    # Save report
    with open(args.output, 'w') as f:
        f.write(report)
    
    print(f"✅ Gas profiling completed!")
    print(f"📊 Report saved to: {args.output}")
    print("\n" + "="*50)
    print(report)

if __name__ == "__main__":
    main()
